// ── Projection Target Codegen ─────────────────────────────────
//
// Handles emission of Expr::Projection — all ProjectionTarget
// variants (Size, Bytes, Contains, BitRange, etc.).
// 2026-06-30: Extracted from rest.rs lines 165-376.

use crate::ast::{Expr, ProjectionTarget, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister};
use std::fmt::Write;

pub fn emit_projection(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    source: &Expr,
    target: &ProjectionTarget,
    indent: &str,
) -> TypedRegister {
    // 2026-07-03: Ptr projection on a function reference emits ptrtoint @fn_name.
    // Must come before try_emit_fn_projection (which handles Address/Name/etc.)
    // because Ptr is not in the Fn* metadata set — it's a runtime pointer value.
    if let ProjectionTarget::Ptr = target {
        if let Expr::Identifier(fn_name) = source {
            if backend.ctx.defn_params.contains_key(fn_name) || backend.ctx.defn_return_types.contains_key(fn_name) {
                writeln!(out, "{}{} = ptrtoint @{} to i64", indent, v, fn_name).ok();
                return TypedRegister { name: v.to_string(), ty: Type::int() };
            }
        }
    }
    // Function metadata projections — source is a function name, not a runtime value.
    // 2026-06-30: Extracted from rest.rs to expr/projection.rs.
    if let Some(result) = backend.try_emit_fn_projection(out, source, target, indent) {
        return result;
    }
    let src_val = backend.emit_expr(out, source, indent);
    // Phase 2: Check if this is a cached projection (Hot Dual path).
    let target_name = crate::analysis::transition_graph::projection_target_name(target);
    if let Some(tr) = backend.try_cached_projection(out, source, &src_val, &target_name, indent) {
        return tr;
    }
    // Phase 2: Check if the source type has a meld route for this projection target.
    if let Some(tr) = backend.try_meld_projection(out, &src_val, &target_name, indent) {
        return tr;
    }
    match target {
        ProjectionTarget::Size => {
            if matches!(source,
                Expr::Decimal(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Char(_))
            {
                writeln!(out, "{}{} = add i64 0, 1", indent, v).ok();
            } else {
                let hp = format!("%php{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                backend.emit_inttoptr(out, indent, &hp, &src_val.name);
                let lp = format!("%plp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, lp, hp).ok();
                writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, v, lp).ok();
            }
        }
        ProjectionTarget::Bytes => {
            let bs = match &src_val.ty {
                Type::Custom(__t) if __t == "Float" => 4,
                Type::Custom(__t) if __t == "Int" || __t == "UInt" => 8,
                Type::Custom(__t) if __t == "Bool" => 1,
                Type::Custom(__t) if __t == "Char" => 4,
                Type::Custom(__t) if __t == "String" || __t == "Data" => 8,
                Type::Custom(name) => {
                    match backend.ctx.struct_types.get(name) {
                        Some(fields) => fields.len() as i64 * 8,
                        None => {
                            panic!("emit_expr: Bytes projection on unknown struct type '{:?}'", name);
                        }
                    }
                }
                _ => {
                    panic!("emit_expr: Bytes projection on unknown type {:?}", src_val.ty);
                }
            };
            writeln!(out, "{}{} = add i64 0, {}", indent, v, bs).ok();
        }
        ProjectionTarget::Ptr => {
            writeln!(out, "{}{} = add i64 0, {} ; ptr", indent, v, src_val.name).ok();
        }
        ProjectionTarget::Alignment => {
            writeln!(out, "{}{} = add i64 0, 8 ; alignment", indent, v).ok();
        }
        ProjectionTarget::Type => {
            let tid = match src_val.ty {
                Type::Custom(__t) if __t == "Int" || __t == "UInt" => 1i64,
                Type::Custom(__t) if __t == "Bool" => 2,
                Type::Custom(__t) if __t == "Char" => 3,
                Type::Custom(__t) if __t == "String" || __t == "Data" => 4,
                Type::Custom(__t) if __t == "Float" => 5,
                Type::Custom(_) => 6,
                Type::Void => 0,
                _ => 0,
            };
            writeln!(out, "{}{} = add i64 0, {} ; type", indent, v, tid).ok();
        }
        ProjectionTarget::Popcount => {
            writeln!(out, "{}{} = call i64 @llvm.ctpop.i64(i64 {})", indent, v, src_val.name).ok();
        }
        ProjectionTarget::LeadingZeros => {
            writeln!(out, "{}{} = call i64 @llvm.ctlz.i64(i64 {}, i1 false)", indent, v, src_val.name).ok();
        }
        ProjectionTarget::TrailingZeros => {
            writeln!(out, "{}{} = call i64 @llvm.cttz.i64(i64 {}, i1 false)", indent, v, src_val.name).ok();
        }
        ProjectionTarget::Absolute => {
            writeln!(out, "{}{} = call i64 @llvm.abs.i64(i64 {}, i1 false)", indent, v, src_val.name).ok();
        }
        ProjectionTarget::BitReverse => {
            writeln!(out, "{}{} = call i64 @llvm.bitreverse.i64(i64 {})", indent, v, src_val.name).ok();
        }
        ProjectionTarget::Keys => {
            writeln!(out, "{}{} = call i64 @__map_keys__(i64 {})", indent, v, src_val.name).ok();
        }
        ProjectionTarget::Values => {
            writeln!(out, "{}{} = call i64 @__map_values__(i64 {})", indent, v, src_val.name).ok();
        }
        ProjectionTarget::AsStack | ProjectionTarget::AsQueue => {
            writeln!(out, "{}{} = add i64 0, {} ; as_stack/as_queue (identity)", indent, v, src_val.name).ok();
        }
        ProjectionTarget::PtrBang => {
            let hp = format!("%pbhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            backend.emit_inttoptr(out, indent, &hp, &src_val.name);
            writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, v, hp).ok();
        }
        ProjectionTarget::Contains(expr) => {
            // Linear search over list elements
            let search_val = backend.emit_expr(out, expr, indent);
            let search_boxed = backend.adapt_to_i64(out, indent, &search_val);
            let hp = format!("%pchp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            backend.emit_inttoptr(out, indent, &hp, &src_val.name);
            let lp = format!("%pclp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, lp, hp).ok();
            let len = format!("%pcln{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, len, lp).ok();
            let dp = format!("%pcdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 2", indent, dp, hp).ok();
            // Emit a linear search loop
            let e_l = format!("pc_entry{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let h_l = format!("pc_hdr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let b_l = format!("pc_body{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let f_l = format!("pc_found{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let d_l = format!("pc_done{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let i_r = format!("%pci{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let c_r = format!("%pcc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let el_r = format!("%pce{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let eq_r = format!("%pceq{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let n_r = format!("%pcn{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}br label %{}", indent, e_l).ok();
            writeln!(out, "{}{}:", indent, e_l).ok();
            writeln!(out, "{}br label %{}", indent, h_l).ok();
            writeln!(out, "{}{}:", indent, h_l).ok();
            writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, i_r, e_l, n_r, b_l).ok();
            writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, c_r, i_r, len).ok();
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, c_r, b_l, d_l).ok();
            writeln!(out, "{}{}:", indent, b_l).ok();
            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, el_r, dp, i_r).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, eq_r, el_r).ok();
            writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, eq_r, eq_r, search_boxed).ok();
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, eq_r, f_l, h_l).ok();
            writeln!(out, "{}{} = add i64 {}, 1", indent, n_r, i_r).ok();
            writeln!(out, "{}br label %{}", indent, h_l).ok();
            writeln!(out, "{}{}:", indent, f_l).ok();
            writeln!(out, "{}br label %{}", indent, d_l).ok();
            writeln!(out, "{}{}:", indent, d_l).ok();
            writeln!(out, "{}{} = phi i1 [ false, %{} ], [ true, %{} ]", indent, v, e_l, f_l).ok();
            return TypedRegister { name: v.to_string(), ty: Type::bool_() };
        }
        ProjectionTarget::Range => {
            // Return list length (same as Size) — Range = [0, len)
            let hp = format!("%prhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            backend.emit_inttoptr(out, indent, &hp, &src_val.name);
            let lp = format!("%prlp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, lp, hp).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, v, lp).ok();
        }
        ProjectionTarget::Top => {
            writeln!(out, "{}{} = call i64 @__stack_top__(i64 {})", indent, v, src_val.name).ok();
        }
        ProjectionTarget::Front => {
            writeln!(out, "{}{} = call i64 @__queue_front__(i64 {})", indent, v, src_val.name).ok();
        }
        ProjectionTarget::Get(expr) => {
            let key_val = backend.emit_expr(out, expr, indent);
            let key_boxed = backend.adapt_to_i64(out, indent, &key_val);
            writeln!(out, "{}{} = call i64 @__hashmap_get__(i64 {}, i64 {})", indent, v, src_val.name, key_boxed).ok();
        }
        ProjectionTarget::Elements => {
            writeln!(out, "{}{} = add i64 0, {} ; elements", indent, v, src_val.name).ok();
        }
        ProjectionTarget::IsEmpty => {
            let hp = format!("%ieh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            backend.emit_inttoptr(out, indent, &hp, &src_val.name);
            let lp = format!("%iel{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, lp, hp).ok();
            let len = format!("%ien{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = load i64, ptr {}, align 8, !tbaa !1", indent, len, lp).ok();
            writeln!(out, "{}{} = icmp eq i64 {}, 0", indent, v, len).ok();
            writeln!(out, "{}{} = zext i1 {} to i64", indent, v, v).ok();
        }
        // ── Phase 2F: Metadata projections ──────────────────────
        ProjectionTarget::Width => {
            let w = src_val.ty.bit_width().unwrap_or(64) as i64;
            writeln!(out, "{}{} = add i64 0, {}", indent, v, w).ok();
        }
        ProjectionTarget::Endian => {
            let endian = backend.ctx.type_universe.as_ref()
                .and_then(|u| u.get_by_type(&src_val.ty))
                .map(|rt| if rt.endian == 0 { "little" } else { "big" })
                .unwrap_or("little");
            writeln!(out, "{}{} = add i64 0, {} ; endian: {}", indent, v, if endian == "big" { 1 } else { 0 }, endian).ok();
        }
        ProjectionTarget::Codec => {
            let codec = backend.ctx.type_universe.as_ref()
                .and_then(|u| u.get_by_type(&src_val.ty))
                .and_then(|rt| rt.codec.as_ref())
                .map(|s| s.as_str())
                .unwrap_or("none");
            writeln!(out, "{}{} = add i64 0, 0 ; codec: {}", indent, v, codec).ok();
        }
        ProjectionTarget::Ops => {
            let count = backend.ctx.type_universe.as_ref()
                .and_then(|u| u.get_by_type(&src_val.ty))
                .map(|rt| rt.operators.len())
                .unwrap_or(0);
            writeln!(out, "{}{} = add i64 0, {} ; ops count", indent, v, count).ok();
        }
        ProjectionTarget::UserDefinedWithArg(name, arg_expr) => {
            // Phase 3.5: Fast-path for well-known operator projections
            if let Some(tr) = backend.try_projection_fast_path(out, &src_val, name.as_str(), arg_expr, indent, &v) {
                return tr;
            }
            panic!("emit_expr: unhandled UserDefinedWithArg projection '{}'", name);
        }
        ProjectionTarget::UserDefined(_) => {
            panic!("emit_expr: unhandled UserDefined projection (no fast-path matched)");
        }
        ProjectionTarget::BitRange(br) => {
            // Extract bits via lshr + and
            let (lo, hi) = match br {
                crate::ast::BitRange::Single(i) => (*i, *i),
                crate::ast::BitRange::Range(l, h) => (*l, *h),
                crate::ast::BitRange::Any(w) => (0, *w - 1),
            };
            let width = hi - lo + 1;
            let shifted = format!("%pbr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = lshr i64 {}, {}", indent, shifted, src_val.name, lo).ok();
            if width >= 64 {
                writeln!(out, "{}{} = add i64 0, {}", indent, v, shifted).ok();
            } else {
                let mask_lit = (1u64 << width) - 1;
                writeln!(out, "{}{} = and i64 {}, {}", indent, v, shifted, mask_lit).ok();
            }
        }
        _ => {
            writeln!(out, "{}{} = add i64 0, 0 ; projection catch-all", indent, v).ok();
        }
    }
    // 2026-06-30: All projection targets that don't explicitly return
    // produce a boxed i64 — return Type::int().
    TypedRegister { name: v.to_string(), ty: Type::int() }
}
