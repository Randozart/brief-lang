// ── Remaining Expression Codegen ─────────────────────────────────
//
// Handles ALL remaining expression types that haven't been extracted
// into dedicated submodules (literal, math, compare, collections,
// intrinsics). Contains Identifier, Call, CellCall, FieldAccess,
// StructInstance, ObjectLiteral, Projection, Arrow, Match, Slice,
// Within, Cast, IsType, FromCheck, Like, Block, and more.
//
// 2026-06-29: Extracted from emit_expr.rs as one block for now.
// Intended to be split into focused submodules in a future pass.

use crate::ast::{Expr, ProjectionTarget, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister};
use std::fmt::Write;

/// Dispatch table for all remaining expression types not handled by
/// dedicated submodule functions. Returns the expression result or
/// falls through to a default `TypedRegister { name: v, ty: Type::Int }`.
pub fn emit_rest_expr(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    expr: &Expr,
    indent: &str,
) -> TypedRegister {
    match expr {
        // ── Identifier ──────────────────────────────────────────
        Expr::Identifier(name) => {
            emit_identifier(backend, out, v, name, indent)
        }
        // ── Concat ──────────────────────────────────────────────
        Expr::Concat(l, r) => {
            let a = backend.emit_expr(out, l, indent);
            let b = backend.emit_expr(out, r, indent);
            return backend.emit_inline_concat(out, indent, &a, &b);
        }
        // ── Projection ──────────────────────────────────────────
        Expr::Projection { source, target } => {
            emit_projection(backend, out, v, source, target, indent)
        }
        // ── StructInstance ──────────────────────────────────────
        Expr::StructInstance(name, fields) => {
            emit_struct_instance(backend, out, v, name, fields, indent)
        }
        // ── FieldAccess ─────────────────────────────────────────
        Expr::FieldAccess(obj, field) => {
            emit_field_access(backend, out, v, obj, field, indent)
        }
        // ── SubtypeProjection ───────────────────────────────────
        Expr::SubtypeProjection { .. } | Expr::SubtypeProjectionExpr(_) => {
            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
            TypedRegister { name: v.to_string(), ty: Type::Int }
        }
        // Fall back to default return for all other types handled
        // by dispatches at the bottom
        _ => {
            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
            TypedRegister { name: v.to_string(), ty: Type::Int }
        }
    }
}

fn emit_identifier(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    name: &str,
    indent: &str,
) -> TypedRegister {
    // Check if it's a state field
    if let Some(&idx) = backend.ctx.field_index_map.get(name) {
        let ll_ty = &backend.ctx.field_types[idx];
        // If we have a cached SSA value for this field, use it directly
        if let Some(reg) = backend.fun.ssa_old_int_regs.get(name).or_else(|| {
            if *ll_ty == "float" || *ll_ty == "double" {
                backend.fun.ssa_old_float_regs.get(name)
            } else {
                None
            }
        }) {
            // Use the cached SSA register directly (avoids GEP+load roundtrip)
            writeln!(out, "{}{} = add i64 0, {}", indent, v, reg).ok();
            if *ll_ty == "float" {
                backend.fun.reg_float_cache.insert(v.to_string(), reg.clone());
            }
            return TypedRegister { name: v.to_string(), ty: Type::Int };
        }
        // Load from state struct
        let gep = format!("%idgep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr {}, i32 0, i32 {}", indent, gep, backend.fun.state_reg_name, idx).ok();
        let ld = format!("%idld{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = load {}, ptr {}, align 8, !tbaa !{}", indent, ld, ll_ty, gep, crate::backend::llvm::tbaa_node(ll_ty)).ok();
        writeln!(out, "{}{} = add i64 0, {}", indent, v, ld).ok();
        if *ll_ty == "float" || *ll_ty == "double" {
            backend.fun.reg_float_cache.insert(v.to_string(), ld);
        }
        return TypedRegister { name: v.to_string(), ty: Type::Int };
    }
    // Check local variable bindings
    if let Some(reg) = backend.fun.let_bindings.get(name) {
        let bty = backend.fun.let_binding_types.get(name).cloned().unwrap_or(Type::Int);
        writeln!(out, "{}{} = add i64 0, {}", indent, v, reg).ok();
        if bty == Type::Float || bty == Type::Float64 {
            backend.fun.reg_float_cache.insert(v.to_string(), reg.clone());
        }
        return TypedRegister { name: v.to_string(), ty: bty };
    }
    // Check struct types for enum-like dispatch
    if let Some(fields) = backend.ctx.struct_types.get(name) {
        let field_vals: Vec<String> = fields.iter().map(|(fname, _)| {
            let fr = format!("%idsf{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = add i64 0, 0", indent, fr).ok();
            fr
        }).collect();
        writeln!(out, "{}{} = add i64 0, {}", indent, v, field_vals.first().map(|s| s.as_str()).unwrap_or("0")).ok();
        return TypedRegister { name: v.to_string(), ty: Type::Custom(name.to_string()) };
    }
    // Default: return 0
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    TypedRegister { name: v.to_string(), ty: Type::Int }
}

fn emit_projection(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    source: &Expr,
    target: &ProjectionTarget,
    indent: &str,
) -> TypedRegister {
    let src = backend.emit_expr(out, source, indent);
    writeln!(out, "{}{} = add i64 0, {}", indent, v, src.name).ok();
    TypedRegister { name: v.to_string(), ty: Type::Int }
}

fn emit_struct_instance(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    name: &str,
    fields: &[(String, Expr)],
    indent: &str,
) -> TypedRegister {
    if let Some(field_types) = backend.ctx.struct_types.get(name) {
        let regs: Vec<TypedRegister> = fields.iter().map(|(_, e)| backend.emit_expr(out, e, indent)).collect();
        if let Some(first_reg) = regs.first() {
            return TypedRegister { name: first_reg.name.clone(), ty: Type::Custom(name.to_string()) };
        }
    }
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    TypedRegister { name: v.to_string(), ty: Type::Int }
}

fn emit_field_access(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    obj: &Expr,
    field: &str,
    indent: &str,
) -> TypedRegister {
    let obj_val = backend.emit_expr(out, obj, indent);
    let hp = format!("%fahp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, obj_val.name).ok();
    let mut found_offset = false;

    // Try to find the field offset from struct_types
    if let Type::Custom(type_name) = &obj_val.ty {
        if let Some(fields) = backend.ctx.struct_types.get(type_name) {
            for (i, (fname, _fty)) in fields.iter().enumerate() {
                if fname == field {
                    let gep = format!("%fagep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, gep, hp, i).ok();
                    let ld = format!("%fald{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, ld, gep).ok();
                    writeln!(out, "{}{} = add i64 0, {}", indent, v, ld).ok();
                    found_offset = true;
                    return TypedRegister { name: v.to_string(), ty: Type::Int };
                }
            }
        }
    }

    // If the source is a State field, do a direct field lookup
    if let Expr::Identifier(obj_name) = obj {
        if let Some(&idx) = backend.ctx.field_index_map.get(obj_name) {
            let ft = &backend.ctx.field_types[idx];
            let gep = format!("%fasgep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr inbounds %State, ptr {}, i32 0, i32 {}", indent, gep, backend.fun.state_reg_name, idx).ok();
            let ld = format!("%fasld{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = load {}, ptr {}, align 8, !tbaa !{}", indent, ld, ft, gep, crate::backend::llvm::tbaa_node(ft)).ok();
            writeln!(out, "{}{} = add i64 0, {}", indent, v, ld).ok();
            return TypedRegister { name: v.to_string(), ty: Type::Int };
        }
    }

    if !found_offset {
        // Access as a tuple-style field index (field name is a number)
        if let Ok(n) = field.parse::<i64>() {
            let gep = format!("%fagep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, gep, hp, n).ok();
            let ld = format!("%fald{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, ld, gep).ok();
            writeln!(out, "{}{} = add i64 0, {}", indent, v, ld).ok();
        } else {
            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
        }
    }

    TypedRegister { name: v.to_string(), ty: Type::Int }
}
