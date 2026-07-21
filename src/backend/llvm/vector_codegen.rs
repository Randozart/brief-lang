use crate::ast::{BinaryOpKind, Expr, Statement, Type};
use crate::backend::llvm::{TypedRegister, LlvmBackend};
use crate::analysis::slp_isomorphism::SlpIsomorphicGroup;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

/// Compute the depth of an expression tree. Used for profitability check.
pub fn tree_depth(expr: &Expr) -> usize {
    match expr {
        Expr::BinaryOp(_, l, r) => 1 + tree_depth(l).max(tree_depth(r)),
        Expr::UnaryOp(_, e) => 1 + tree_depth(e),
        _ => 0,
    }
}

/// Compute the LLVM vector type string for a given element type and width.
/// Examples: "<3 x float>", "<5 x float>", "<4 x i64>"
fn vector_type_str(element_type: &Type, width: usize) -> String {
    let el = if *element_type == Type::float64() {
        "double"
    } else if *element_type == Type::float() {
        "float"
    } else {
        "i64"
    };
    format!("<{} x {}>", width, el)
}

/// Convert a scalar register to a different LLVM type if needed.
/// Returns the register name (converted or original).
fn convert_scalar_type(
    backend: &mut LlvmBackend,
    out: &mut String,
    reg_name: &str,
    reg_ty: &Type,
    target_llvm_ty: &str,
    indent: &str,
) -> String {
    let src_llvm = backend.llvm_type(reg_ty);
    if src_llvm == target_llvm_ty {
        return reg_name.to_string();
    }
    // i64 → float: trunc to i32, bitcast to float
    if src_llvm == "i64" && target_llvm_ty == "float" {
        let tr = backend.fun.next_reg_with_prefix("cvt");
        let fl = backend.fun.next_reg_with_prefix("cvf");
        writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, reg_name).ok();
        writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
        return fl;
    }
    // i64 → double: bitcast i64 to double
    if src_llvm == "i64" && target_llvm_ty == "double" {
        let dbl = backend.fun.next_reg_with_prefix("cvd");
        writeln!(out, "{}{} = bitcast i64 {} to double", indent, dbl, reg_name).ok();
        return dbl;
    }
    // float → i64: bitcast float to i32, zext to i64
    if src_llvm == "float" && target_llvm_ty == "i64" {
        let b32 = backend.fun.next_reg_with_prefix("cvi");
        let z64 = backend.fun.next_reg_with_prefix("cvZ");
        writeln!(out, "{}{} = bitcast float {} to i32", indent, b32, reg_name).ok();
        writeln!(out, "{}{} = zext i32 {} to i64", indent, z64, b32).ok();
        return z64;
    }
    reg_name.to_string()
}

/// Mask type for shufflevector — always <width x i32>.
fn mask_type_str(width: usize) -> String {
    format!("<{} x i32>", width)
}

/// Check if a template identifier maps to the same variable across all lanes.
/// If true, we can broadcast (splat) the value to all lanes.
fn all_lanes_same(name: &str, lane_mappings: &[HashMap<String, String>]) -> bool {
    for mapping in lane_mappings.iter().skip(1) {
        match mapping.get(name) {
            Some(mapped) if mapped == name => {}
            _ => return false,
        }
    }
    true
}

/// Check if a template identifier is shared (maps to same name in all lanes)
/// OR if it maps to different names in different lanes.
fn is_same_across_lanes(name: &str, lane_mappings: &[HashMap<String, String>]) -> bool {
    let mut seen: Option<&str> = None;
    for mapping in lane_mappings {
        let mapped = mapping.get(name).map(|s| s.as_str()).unwrap_or(name);
        match seen {
            None => seen = Some(mapped),
            Some(s) if s != mapped => return false,
            _ => {}
        }
    }
    true
}

// ── Expression Tree Walker ───────────────────────────────────────

/// Recursively emit a vector expression for an SLP group.
/// `template_expr` is the RHS expression from the first statement in the group.
/// `lane_exprs` are the RHS expressions from each lane (same structure as template).
/// `lane_mappings` maps template variable names → per-lane variable names.
/// `width` is the number of lanes.
/// Returns a TypedRegister for the vector result.
fn emit_vector_expr(
    backend: &mut LlvmBackend,
    out: &mut String,
    template_expr: &Expr,
    lane_exprs: &[&Expr],
    lane_mappings: &[HashMap<String, String>],
    width: usize,
    indent: &str,
) -> Result<TypedRegister, String> {
    let vec_ty_str = || vector_type_str(&Type::float(), width);

    match (template_expr, lane_exprs.first()) {
        // ── BinaryOp ──────────────────────────────────────────
        (Expr::BinaryOp(kind, t_l, t_r), Some(_)) => {
            let lhs_lanes: Vec<&Expr> = lane_exprs.iter().map(|e| {
                if let Expr::BinaryOp(_, l, _) = e { &**l } else { &**t_l }
            }).collect();
            let rhs_lanes: Vec<&Expr> = lane_exprs.iter().map(|e| {
                if let Expr::BinaryOp(_, _, r) = e { &**r } else { &**t_r }
            }).collect();

            let lhs = emit_vector_expr(backend, out, t_l, &lhs_lanes, lane_mappings, width, indent)?;
            let rhs = emit_vector_expr(backend, out, t_r, &rhs_lanes, lane_mappings, width, indent)?;

            let v = backend.fun.next_reg_with_prefix("sv");
            let vty = vector_type_str(&lhs.ty, width);
            let op = match kind {
                BinaryOpKind::Add => {
                    let is_fp = lhs.ty == Type::float() || lhs.ty == Type::float64();
                    if is_fp { format!("fadd {} {}, {}", vty, lhs.name, rhs.name) }
                    else { format!("add nuw nsw {} {}, {}", vty, lhs.name, rhs.name) }
                },
                BinaryOpKind::Sub => {
                    let is_fp = lhs.ty == Type::float() || lhs.ty == Type::float64();
                    if is_fp { format!("fsub {} {}, {}", vty, lhs.name, rhs.name) }
                    else { format!("sub nsw {} {}, {}", vty, lhs.name, rhs.name) }
                },
                BinaryOpKind::Mul => {
                    let is_fp = lhs.ty == Type::float() || lhs.ty == Type::float64();
                    if is_fp { format!("fmul {} {}, {}", vty, lhs.name, rhs.name) }
                    else { format!("mul nsw {} {}, {}", vty, lhs.name, rhs.name) }
                },
                BinaryOpKind::Div => {
                    let is_fp = lhs.ty == Type::float() || lhs.ty == Type::float64();
                    if is_fp { format!("fdiv {} {}, {}", vty, lhs.name, rhs.name) }
                    else { format!("sdiv {} {}, {}", vty, lhs.name, rhs.name) }
                },
                BinaryOpKind::Eq => format!("icmp eq {} {}, {}", vty, lhs.name, rhs.name),
                BinaryOpKind::Neq => format!("icmp ne {} {}, {}", vty, lhs.name, rhs.name),
                BinaryOpKind::Lt => format!("icmp slt {} {}, {}", vty, lhs.name, rhs.name),
                BinaryOpKind::Gt => format!("icmp sgt {} {}, {}", vty, lhs.name, rhs.name),
                BinaryOpKind::Le => format!("icmp sle {} {}, {}", vty, lhs.name, rhs.name),
                BinaryOpKind::Ge => format!("icmp sge {} {}, {}", vty, lhs.name, rhs.name),
                _ => return Err(format!("unsupported BinaryOpKind for vectorization: {:?}", kind)),
            };
            writeln!(out, "{}{} = {}", indent, v, op).ok();
            Ok(TypedRegister { name: v, ty: lhs.ty })
        }

        // ── Identifier ────────────────────────────────────────
        (Expr::Identifier(name), _) => {
            let vty = vec_ty_str();
            let el_llvm_ty = vty.split(' ').last()
                .map(|s| s.trim_end_matches('>').trim())
                .unwrap_or("float").to_string();
            if is_same_across_lanes(name, lane_mappings) {
                // All lanes reference the same variable — broadcast
                let scalar = backend.emit_expr(out, template_expr, indent);
                let conv = convert_scalar_type(backend, out, &scalar.name, &scalar.ty, &el_llvm_ty, indent);
                let v = backend.fun.next_reg_with_prefix("sbc");
                writeln!(out, "{}{} = insertelement {} undef, {} {}, i32 0",
                    indent, v, vty, el_llvm_ty, conv).ok();
                let shuf = backend.fun.next_reg_with_prefix("sbs");
                let mty = mask_type_str(width);
                writeln!(out, "{}{} = shufflevector {} {}, {} undef, {} zeroinitializer",
                    indent, shuf, vty, v, vty, mty).ok();
                Ok(TypedRegister { name: shuf, ty: scalar.ty })
            } else {
                // Different per lane — insertelement chain
                let mut vec_name = String::new();
                for i in 0..width {
                    let lane_name = lane_mappings[i].get(name).cloned().unwrap_or_else(|| name.to_string());
                    let lane_expr = Expr::Identifier(lane_name);
                    let scalar = backend.emit_expr(out, &lane_expr, indent);
                    let conv = convert_scalar_type(backend, out, &scalar.name, &scalar.ty, &el_llvm_ty, indent);
                    if i == 0 {
                        let v0 = backend.fun.next_reg_with_prefix("sie");
                        writeln!(out, "{}{} = insertelement {} undef, {} {}, i32 0",
                            indent, v0, vty, el_llvm_ty, conv).ok();
                        vec_name = v0;
                    } else {
                        let vn = backend.fun.next_reg_with_prefix("sie");
                        writeln!(out, "{}{} = insertelement {} {}, {} {}, i32 {}",
                            indent, vn, vty, vec_name, el_llvm_ty, conv, i).ok();
                        vec_name = vn;
                    }
                }
                Ok(TypedRegister { name: vec_name, ty: Type::float() })
            }
        }

        // ── Literals ──────────────────────────────────────────
        // Emit the scalar literal using emit_expr (handles float/double hex format,
        // decimal, bool) then broadcast to all lanes.
        (Expr::Float(_) | Expr::Decimal(_) | Expr::Bool(_), _) => {
            let scalar = backend.emit_expr(out, template_expr, indent);
            let v = backend.fun.next_reg_with_prefix("slb");
            let vty = vector_type_str(&scalar.ty, width);
            writeln!(out, "{}{} = insertelement {} undef, {} {}, i32 0",
                indent, v, vty, backend.llvm_type(&scalar.ty), scalar.name).ok();
            let shuf = backend.fun.next_reg_with_prefix("sls");
            let mty = mask_type_str(width);
            writeln!(out, "{}{} = shufflevector {} {}, {} undef, {} zeroinitializer",
                indent, shuf, vty, v, vty, mty).ok();
            Ok(TypedRegister { name: shuf, ty: scalar.ty })
        }

        // ── Non-vectorizable fallback ─────────────────────────
        _ => {
            // Fall back: emit each lane as scalar, build vector from results
            let mut vec_name = String::new();
            for i in 0..width {
                let lane_expr = lane_exprs.get(i).copied().unwrap_or(template_expr);
                let scalar = backend.emit_expr(out, lane_expr, indent);
                if i == 0 {
                    let v0 = backend.fun.next_reg_with_prefix("sfc");
                    writeln!(out, "{}{} = insertelement {} undef, {} {}, i32 0",
                        indent, v0, vec_ty_str(), backend.llvm_type(&scalar.ty), scalar.name).ok();
                    vec_name = v0;
                } else {
                    let vn = backend.fun.next_reg_with_prefix("sfc");
                    writeln!(out, "{}{} = insertelement {} {}, {} {}, i32 {}",
                        indent, vn, vec_ty_str(), vec_name, backend.llvm_type(&scalar.ty), scalar.name, i).ok();
                    vec_name = vn;
                }
            }
            // Determine result type from the first lane's type
            let first_scalar = backend.emit_expr(out, &Expr::Decimal(0), indent);
            Ok(TypedRegister { name: vec_name, ty: first_scalar.ty })
        }
    }
}

/// Emit extractelement for all lanes of an SLP group and register results.
fn emit_extract_and_register(
    backend: &mut LlvmBackend,
    out: &mut String,
    vec_reg: &str,
    vec_ty: &Type,
    group: &SlpIsomorphicGroup,
    body: &[crate::ast::Statement],
    write_set: &HashSet<String>,
    indent: &str,
) {
    let vty = vector_type_str(vec_ty, group.width);

    for i in 0..group.width {
        let ex = backend.fun.next_reg_with_prefix("sex");
        writeln!(out, "{}{} = extractelement {} {}, i32 {}",
            indent, ex, vty, vec_reg, i).ok();

        let lhs_name = &group.lhs_names[i];
        let scalar_type = if *vec_ty == Type::float64() {
            Type::float64()
        } else if *vec_ty == Type::float() {
            Type::float()
        } else {
            Type::int()
        };

        // Register in last_val_temps
        backend.fun.last_val_temps.insert(lhs_name.clone(), ex.clone());
        backend.fun.last_val_types.insert(lhs_name.clone(), scalar_type.clone());

        // Handle write_set (phi backedge) and state stores.
        // The target name is always group.lhs_names[i] — works for both
        // contiguous and cross-pair merged groups.
        let target_name = &group.lhs_names[i];
        if write_set.contains(target_name) {
                let field_ty = backend.ctx.field_index_map.get(target_name)
                    .and_then(|idx| backend.ctx.field_types.get(*idx))
                    .cloned().unwrap_or_else(|| "i64".to_string());
                if field_ty == "float" || field_ty == "double" {
                    backend.fun.pending_phi_backedge.insert(target_name.clone(), ex.clone());
                } else {
                    let boxed = backend.adapt_to_i64(out, indent, &TypedRegister {
                        name: ex.clone(),
                        ty: scalar_type.clone(),
                    });
                    backend.fun.pending_phi_backedge.insert(target_name.clone(), boxed);
                }
            }
            if backend.fun.needs_state_stores_in_body {
                if let Some(&idx) = backend.ctx.field_index_map.get(target_name) {
                    let field_ty = &backend.ctx.field_types[idx].clone();
                    let gep = backend.fun.next_reg_with_prefix("svs");
                    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}",
                        indent, gep, idx).ok();
                    writeln!(out, "{}{} = store {} {}, ptr {}, align 8",
                        indent, "", field_ty, ex, gep).ok();
                }
        }
    }
}

// ── Entry Point ──────────────────────────────────────────────────

/// Emit vector operations for an entire SLP group.
/// Called from emit_countable_body when a statement at body[i] matches
/// group.base_index. Emits vector ops for all lanes, then i += group.width.
/// Check if any lane in the group depends on a previous lane's output.
/// Sequential dependencies prevent SLP vectorization (Newton iteration case).
fn has_lane_dependency(body: &[Statement], group: &SlpIsomorphicGroup) -> bool {
    let mut prev_lhs: HashSet<String> = HashSet::new();
    for i in 0..group.width {
        let stmt = match body.get(group.base_index + i) {
            Some(s) => s,
            None => return true,
        };
        let rhs = match stmt {
            Statement::Let { expr: Some(e), .. } => e,
            Statement::Assign(_, e) => e,
            _ => return true,
        };
        // Check if this lane's RHS references any previous lane's LHS
        fn expr_refs(expr: &Expr, targets: &HashSet<String>) -> bool {
            match expr {
                Expr::Identifier(n) => targets.contains(n),
                Expr::BinaryOp(_, l, r) => expr_refs(l, targets) || expr_refs(r, targets),
                Expr::UnaryOp(_, e) => expr_refs(e, targets),
                _ => false,
            }
        }
        if expr_refs(rhs, &prev_lhs) {
            return true;
        }
        // Add this lane's LHS to the set
        match stmt {
            Statement::Let { name, .. } => { prev_lhs.insert(name.clone()); }
            Statement::Assign(lhs, _) => {
                if let Expr::Identifier(n) = &*lhs { prev_lhs.insert(n.clone()); }
            }
            _ => {}
        }
    }
    false
}

pub fn emit_slp_group(
    backend: &mut LlvmBackend,
    out: &mut String,
    body: &[Statement],
    group: &SlpIsomorphicGroup,
    write_set: &HashSet<String>,
) -> Result<(), String> {
    // Skip groups with sequential lane dependencies (Newton iteration case)
    if has_lane_dependency(body, group) {
        return Err("SLP group has sequential lane dependencies".to_string());
    }

    let template_stmt = body.get(group.base_index)
        .ok_or_else(|| "SLP group base_index out of bounds".to_string())?;
    let template_expr = match template_stmt {
        Statement::Let { expr: Some(e), .. } => &*e,
        Statement::Assign(_, e) => &*e,
        _ => return Err("SLP group template is not Let or Assign".to_string()),
    };

    // 2026-07-21: All lanes use the template expression. lane_mappings[i]
    // provides per-lane variable name substitutions. This works for both
    // contiguous and cross-pair merged groups — the template is always the
    // first statement of the group, and lane_mappings encode the differences.
    let lane_exprs: Vec<&Expr> = (0..group.width).map(|_| template_expr).collect();

    let vec_result = emit_vector_expr(
        backend, out, template_expr, &lane_exprs,
        &group.lane_mappings, group.width, "  ")?;

    emit_extract_and_register(
        backend, out, &vec_result.name, &vec_result.ty,
        group, body, write_set, "  ");

    Ok(())
}
