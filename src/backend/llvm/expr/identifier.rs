// ── Identifier Variable Access Codegen ─────────────────────────
//
// Handles emission of Expr::Identifier, Expr::AddrOf, and
// Expr::PriorState — variable and field lookups.
// 2026-06-29: Extracted from emit_expr.rs lines 109-430.

use crate::ast::{Expr, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister};
use std::fmt::Write;

/// Emit an Identifier expression (variable/field lookup).
pub fn emit_identifier(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    expr: &Expr,
    indent: &str,
) -> TypedRegister {
    // ── Expr::PriorState ────────────────────────────────────
    // 2026-06-30: PriorState MUST read from ssa_state_reg (pre-tick value),
    // NOT from let_bindings, triggers, or constants (current-tick values).
    // Early-return before the Identifier path.
    if matches!(expr, Expr::PriorState(_)) {
        let name = match expr { Expr::PriorState(n) => n, _ => "" };
        if let Some(&idx) = backend.ctx.field_index_map.get(name) {
            let ll_ty = &backend.ctx.field_types[idx];
            let ev = format!("%pev{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            if let Some(ref ssa_reg) = backend.fun.ssa_state_reg.clone() {
                writeln!(out, "{}{} = extractvalue %State {}, {}", indent, ev, ssa_reg, idx).ok();
                let field_ty = match ll_ty.as_str() {
                    "i8" => {
                        let tr = format!("%ptr_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i8 {} to i1", indent, tr, ev).ok();
                        return TypedRegister { name: tr, ty: Type::Custom("Bool".to_string()) };
                    }
                    "i32" => {
                        let z = format!("%piz_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, z, ev).ok();
                        writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok();
                        return TypedRegister { name: v.to_string(), ty: Type::Custom("Char".to_string()) };
                    }
                    "float" => {
                        return TypedRegister { name: ev, ty: Type::Custom("Float".to_string()) };
                    }
                    _ => {
                        writeln!(out, "{}{} = add i64 0, {}", indent, v, ev).ok();
                        return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
                    }
                };
            }
        }
        panic!("emit_expr: PriorState field '{}' not found in field_index_map", name);
    }

    // ── Expr::Identifier / Expr::AddrOf ────────────────────────
    let name = match expr {
        Expr::Identifier(n) => n,
        expr @ Expr::AddrOf(_) => expr.as_var_name().unwrap(),
        Expr::PriorState(n) => n,
        _ => { writeln!(out, "{}{} = add i64 0, 0", indent, v).ok(); return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }; }
    };
    // SSA body mode: prefer pre-extracted old-value register
    // for int fields so all body ops are independent.
    if let Some(old_reg) = backend.fun.ssa_old_int_regs.get(name) {
        // If the old register is a non-i64 type, cast to i64 first
        if let Some(&idx) = backend.ctx.field_index_map.get(name) {
            let ft = &backend.ctx.field_types[idx];
            if ft == "i8" {
                let z = format!("%iz_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = trunc i8 {} to i1", indent, z, old_reg).ok();
                return TypedRegister { name: z, ty: Type::Custom("Bool".to_string()) };
            }
            if ft == "i32" {
                let z = format!("%iz_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = zext i32 {} to i64", indent, z, old_reg).ok();
                writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok();
                // i32 LLVM type means Char at the Brief level
                // (the only Brief type mapped to i32).
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Char".to_string()) };
            }
            if ft == "i8*" || ft == "ptr" {
                // old_reg is i8* from extractvalue on state (state stores
                // native i8* for String fields, not boxed i64). ptrtoint to box.
                writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, old_reg).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
            }
        }
        writeln!(out, "{}{} = add i64 0, {}", indent, v, old_reg).ok();
        return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
    }
    // SSA body mode: prefer pre-extracted old-value register
    // for float fields so all body ops are independent.
    // 2026-06-29: Check field type to return Float (float) or Float64 (double).
    if let Some(old_reg) = backend.fun.ssa_old_float_regs.get(name) {
        backend.fun.reg_float_cache.insert(old_reg.clone(), old_reg.clone());
        let brief_ty = if let Some(&idx) = backend.ctx.field_index_map.get(name) {
            let ft = &backend.ctx.field_types[idx];
            if ft == "double" { Type::Custom("Float64".to_string()) } else { Type::Custom("Float".to_string()) }
        } else {
            Type::Custom("Float".to_string())
        };
        return TypedRegister { name: old_reg.to_string(), ty: brief_ty };
    }
    if let Some(ref ssa_reg) = backend.fun.ssa_state_reg.clone() {
    if let Some(&addr) = backend.ctx.mmio_fields.get(name) {
        let p = format!("%gep_exit_{}", backend.fun.txn_counter);
        backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, addr).ok();
        writeln!(out, "{}{} = load volatile i64, ptr {}, align 1", indent, v, p).ok();
    } else if let Some(&idx) = backend.ctx.field_index_map.get(name) {
            let ll_ty = &backend.ctx.field_types[idx];
            let brief_ty = backend.ctx.field_brief_types.get(idx).cloned().unwrap_or(Type::Custom("Int".to_string()));
            let ev = format!("%ev{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = extractvalue %State {}, {}", indent, ev, ssa_reg, idx).ok();
            // 2026-06-29: Use field_brief_types to restore the correct Brief type.
            // This handles Char→"i32", Int32→"i32" etc. correctly.
            if brief_ty == Type::Custom("Bool".to_string()) {
                let tr = format!("%tr_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = trunc i8 {} to i1", indent, tr, ev).ok();
                return TypedRegister { name: tr, ty: Type::Custom("Bool".to_string()) };
            }
            if brief_ty == Type::Custom("Float".to_string()) {
                let fc = backend.fun.txn_counter; backend.fun.txn_counter += 1;
                let float_reg = format!("%flt_{}_{}", name, fc);
                writeln!(out, "{}{} = extractvalue %State {}, {}", indent, float_reg, ssa_reg, idx).ok();
                backend.fun.reg_float_cache.insert(float_reg.clone(), float_reg.clone());
                return TypedRegister { name: float_reg, ty: Type::Custom("Float".to_string()) };
            }
            if brief_ty == Type::Custom("Float64".to_string()) {
                let fc = backend.fun.txn_counter; backend.fun.txn_counter += 1;
                let float_reg = format!("%flt_{}_{}", name, fc);
                writeln!(out, "{}{} = extractvalue %State {}, {}", indent, float_reg, ssa_reg, idx).ok();
                return TypedRegister { name: float_reg, ty: Type::Custom("Float64".to_string()) };
            }
            if brief_ty == Type::Custom("Char".to_string()) {
                writeln!(out, "{}{} = zext i32 {} to i64", indent, v, ev).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Char".to_string()) };
            }
            if brief_ty == Type::Custom("String".to_string()) || brief_ty == Type::Custom("Data".to_string()) {
                writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, ev).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
            }
            // 2026-06-29: Fixed-width integer types — retain Brief type
            if brief_ty == Type::Custom("Int8".to_string()) || brief_ty == Type::Custom("UInt8".to_string()) {
                writeln!(out, "{}{} = add i8 0, {}", indent, v, ev).ok();
                return TypedRegister { name: v.to_string(), ty: brief_ty };
            }
            if brief_ty == Type::Custom("Int16".to_string()) || brief_ty == Type::Custom("UInt16".to_string()) {
                writeln!(out, "{}{} = add i16 0, {}", indent, v, ev).ok();
                return TypedRegister { name: v.to_string(), ty: brief_ty };
            }
            if brief_ty == Type::Custom("Int32".to_string()) || brief_ty == Type::Custom("UInt32".to_string()) {
                writeln!(out, "{}{} = add i32 0, {}", indent, v, ev).ok();
                return TypedRegister { name: v.to_string(), ty: brief_ty };
            }
            {
                writeln!(out, "{}{} = add i64 0, {}", indent, v, ev).ok();
                return TypedRegister { name: v.to_string(), ty: brief_ty };
            }
        }
    }
    if let Some(reg) = backend.fun.let_bindings.get(name) {
        if let Some(ty) = backend.fun.let_binding_types.get(name) {
            if *ty == Type::Custom("Float".to_string()) {
                return TypedRegister { name: reg.clone(), ty: Type::Custom("Float".to_string()) };
            }
            // 2026-06-29: Float64 let-binding — return native double register
            if *ty == Type::Custom("Float64".to_string()) {
                return TypedRegister { name: reg.clone(), ty: Type::Custom("Float64".to_string()) };
            }
            if *ty == Type::Custom("Char".to_string()) {
                // All Char registers from emit_expr are already i64.
                // Copy the register as-is; no zext needed.
                writeln!(out, "{}{} = add i64 0, {}", indent, v, reg).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Char".to_string()) };
            }
            if *ty == Type::Custom("Bool".to_string()) {
                let z = format!("%iz_b{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = zext i1 {} to i64", indent, z, reg).ok();
                writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
            }
        }
        writeln!(out, "{}{} = add i64 0, {}", indent, v, reg).ok();
        if let Some(ty) = backend.fun.let_binding_types.get(name) {
            return TypedRegister { name: v.to_string(), ty: ty.clone() };
        }
    }
    if backend.ctx.trigger_names.contains(&name.to_string()) {
        if let Some(sampled) = backend.sampled_triggers.get(name) {
            writeln!(out, "{}{} = add i64 0, {}", indent, v, sampled).ok();
            return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
        } else if let Some(t) = backend.ctx.triggers.get(name).cloned() {
            // For built-in triggers (@stdin#, @timer#, @signal#), load from
            // the state field (the event loop stored the value there).
            if matches!(t.address, crate::ast::LinkRef::Stdin | crate::ast::LinkRef::Timer(_) | crate::ast::LinkRef::Signal(_)) {
                if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                    let ll_ty = &backend.ctx.field_types[idx];
                    let sge = format!("%sge_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, sge, idx).ok();
                    let ev = format!("%ev_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    match ll_ty.as_str() {
                        "i8" => { writeln!(out, "{}{} = load i8, ptr {}, align 1", indent, ev, sge).ok(); }
                        "i32" => { writeln!(out, "{}{} = load i32, ptr {}, align 4", indent, ev, sge).ok(); }
                        _ => { writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ev, sge).ok(); }
                    }
                    backend.emit_trg_load_finish(out, indent, &v, ev, &t.ty);
                    return TypedRegister { name: v.to_string(), ty: t.ty.clone() };
                } else {
                    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                    return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
                }
            } else if matches!(t.address, crate::ast::LinkRef::Explicit(0)) {
                // Cell-binding triggers (trg name @ Console!) and other
                // Explicit(0) triggers load from the %State field.
                if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                    let ll_ty = &backend.ctx.field_types[idx];
                    let sge = format!("%sge_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, sge, idx).ok();
                    let ev = format!("%ev_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    match ll_ty.as_str() {
                        "i8" => { writeln!(out, "{}{} = load i8, ptr {}, align 1", indent, ev, sge).ok(); }
                        "i32" => { writeln!(out, "{}{} = load i32, ptr {}, align 4", indent, ev, sge).ok(); }
                        _ => { writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ev, sge).ok(); }
                    }
                    // 2026-06-28: String/Data types are boxed as i64 in %State.
                    // emit_trg_load_finish expects i8* for String; convert here.
                    if t.ty == Type::Custom("String".to_string()) || t.ty == Type::Custom("Data".to_string()) {
                        let ip = format!("%tip_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ip, ev).ok();
                        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, ip).ok();
                    } else {
                        backend.emit_trg_load_finish(out, indent, &v, ev, &t.ty);
                    }
                    return TypedRegister { name: v.to_string(), ty: t.ty.clone() };
                } else {
                    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                    return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
                }
            } else {
                backend.emit_trg_load(out, indent, &v, &t.address, &t.ty);
                return TypedRegister { name: v.to_string(), ty: t.ty.clone() };
            }
        } else {
            writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
            return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
        }
    } else if let Some((ty, expr)) = backend.ctx.constants.get(name) {
        // Inline literal integer/bool constants as immediates
        // instead of loading from global RAM.
        match (ty, expr) {
            (Type::Custom(__t), Expr::Integer(n)) if __t == "Int" || __t == "UInt" => {
                writeln!(out, "{}{} = add i64 0, {}", indent, v, n).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
            }
            (Type::Custom(__t), Expr::Bool(b)) if __t == "Bool" => {
                if *b {
                    writeln!(out, "{}{} = and i1 true, true", indent, v).ok();
                } else {
                    writeln!(out, "{}{} = xor i1 true, true", indent, v).ok();
                }
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Bool".to_string()) };
            }
            _ => {
                // 2026-06-29: Handle Float64 constant loading (load as double, return native)
                if *ty == Type::Custom("Float64".to_string()) {
                    writeln!(out, "{}{} = load double, ptr @{}, align 8", indent, v, name).ok();
                    backend.fun.reg_float_cache.insert(v.to_string(), v.to_string());
                    return TypedRegister { name: v.to_string(), ty: Type::Custom("Float64".to_string()) };
                }
                let ll_ty = match ty {
                    Type::Custom(__t) if __t == "Float" => "float",
                    Type::Custom(__t) if __t == "Int" || __t == "UInt" => "i64",
                    Type::Custom(__t) if __t == "Bool" => "i8",
                    _ => "i64",
                };
                let ld = format!("%il{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load {}, {}* @{}, align {}", indent, ld, ll_ty, ll_ty, name, backend.align_of(ll_ty)).ok();
                let ret_ty = match ty {
                    Type::Custom(__t) if __t == "Float" => {
                        backend.fun.reg_float_cache.insert(ld.clone(), ld.clone());
                        return TypedRegister { name: ld.clone(), ty: Type::Custom("Float".to_string()) };
                    }
                    Type::Custom(__t) if __t == "Bool" => {
                        let z = format!("%iz_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i8 {} to i1", indent, z, ld).ok();
                        return TypedRegister { name: z, ty: Type::Custom("Bool".to_string()) };
                    }
                    _ => {
                        writeln!(out, "{}{} = add i64 0, {}", indent, v, ld).ok();
                        ty.clone()
                    }
                };
                return TypedRegister { name: v.to_string(), ty: ret_ty };
            }
        }
    } else if let Some(&addr) = backend.ctx.mmio_fields.get(name) {
        let p = format!("%mio{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, addr).ok();
        writeln!(out, "{}{} = load volatile i64, ptr {}, align 1", indent, v, p).ok();
    } else if let Some(&idx) = backend.ctx.field_index_map.get(name) {
        let ty = &backend.ctx.field_types[idx];
        let p = format!("%fdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = getelementptr inbounds %State, ptr %state, i32 0, i32 {}", indent, p, idx).ok();
        let rng = backend.ctx.field_to_meta_idx.get(name).map(|m| format!(", !range !{}", m)).unwrap_or_default();
        match ty {
            s if s == "i8" => {
                writeln!(out, "{}{} = load i8, ptr {}, align {}", indent, v, p, backend.align_of("i8")).ok();
                let tr = format!("%tr_{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = trunc i8 {} to i1", indent, tr, v).ok();
                return TypedRegister { name: tr, ty: Type::Custom("Bool".to_string()) };
            }
            s if s == "float" => {
                writeln!(out, "{}{} = load float, ptr {}, align 4", indent, v, p).ok();
                backend.fun.reg_float_cache.insert(v.to_string(), v.to_string());
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Float".to_string()) };
            }
            s if s == "double" => {
                // 2026-06-29: Float64 field reads — load double, return Float64
                writeln!(out, "{}{} = load double, ptr {}, align 8", indent, v, p).ok();
                backend.fun.reg_float_cache.insert(v.to_string(), v.to_string());
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Float64".to_string()) };
            }
            s if s == "i8*" => {
                let ld = format!("%ild{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i8*, i8** {}, align 8", indent, ld, p).ok();
                writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, ld).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
            }
            s if s == "i32" => {
                let ld = format!("%il{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i32, ptr {}, align 4", indent, ld, p).ok();
                writeln!(out, "{}{} = zext i32 {} to i64", indent, v, ld).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Char".to_string()) };
            }
            _ => {
                writeln!(out, "{}{} = load {}, {}* {}, align {}{}", indent, v, ty, ty, p, backend.align_of(ty), rng).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
            }
        }
    } else {
        writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    }
    // Default fallthrough
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
}

/// Emit a GEP (getelementptr) to compute the address of a variable.
/// Returns the register name holding the pointer (`ptr` in LLVM IR).
///
/// Supported cases:
/// - `Expr::Identifier(name)`: state field (GEP on `%State*`)
///
/// Phase 2: Currently only handles state fields. Let binding and
/// FieldAccess/Index targets are future extensions.
pub(super) fn emit_addr_of(
    backend: &mut crate::backend::llvm::LlvmBackend,
    out: &mut String,
    expr: &Expr,
    indent: &str,
) -> Result<String, String> {
    match expr {
        Expr::Identifier(name) => {
            if let Some(&idx) = backend.ctx.field_index_map.get(name) {
                // State field → GEP on %State*
                let state_ptr = &backend.fun.state_reg_name;
                let reg = format!("%ap{}", backend.fun.txn_counter);
                backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr inbounds %State, ptr %{}, i32 0, i32 {}",
                    indent, reg, state_ptr, idx)
                    .map_err(|e| e.to_string())?;
                Ok::<String, String>(reg)
            } else {
                // Let bindings without an alloca are not supported.
                // Phase 2 extension: create alloca on demand.
                Err(format!("cannot take address of '{}': not a state field", name))
            }
        }
        _ => Err("cannot take address of expression: only identifiers supported".to_string()),
    }
}

