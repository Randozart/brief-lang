// ── Function/FFI Call Codegen ────────────────────────────────
//
// Handles emission of Expr::Call — both FFI (frgn) and internal
// (defn/txn) function calls, including enum variant construction.
// 2026-06-30: Extracted from rest.rs lines 115-345.

use crate::ast::{Expr, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister};
use std::fmt::Write;

pub fn emit_call(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    name: &str,
    args: &[Expr],
    indent: &str,
) -> TypedRegister {
    // 2026-06-17: Inline negated (stdlib projection, not defined as a function)
    // 2026-06-30: Extracted from rest.rs to expr/call.rs.
    // 2026-07-01: Handle Float/Float64 types — emit fneg instead of sub i64 0.
    // The negated intrinsic is called from stdlib when T::neg() is invoked on
    // a generic type that resolves to Float or Float64. Previously all types
    // got `sub i64 0, %val` which was a type error for float types.
    if name == "negated" && args.len() >= 1 {
        let val = backend.emit_expr(out, &args[0], indent);
        match val.ty {
            Type::Custom(__t) if __t == "Float" => {
                writeln!(out, "{}{} = fneg float {}", indent, v, val.name).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Float".to_string()) };
            }
            Type::Custom(__t) if __t == "Float64" => {
                writeln!(out, "{}{} = fneg double {}", indent, v, val.name).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Float64".to_string()) };
            }
            _ => {
                writeln!(out, "{}{} = sub i64 0, {}", indent, v, val.name).ok();
                return TypedRegister { name: v.to_string(), ty: val.ty };
            }
        }
    }
    // Clone foreign info upfront to avoid borrow conflict with emit_expr
    // 2026-07-03: Check if this is an indirect call through a function pointer variable.
    if let Some(tr) = try_fn_ptr_call(backend, out, v, name, args, indent) {
        return tr;
    }
    let frgn_sig: Option<(Vec<(String, Type)>, crate::ast::ResultType, bool, Option<crate::ast::Expr>, Vec<(String, Type)>)> =
        backend.ctx.frgn_map.get(name).map(|s| (s.inputs.clone(), s.result_type.clone(), s.is_pipe, s.fallback.clone(), s.success_output.clone()));
    if let Some((inputs, ret_type, is_pipe, fallback, success_output)) = frgn_sig {
        let mut marshaled: Vec<String> = Vec::new();
        for (i, (_, arg_ty)) in inputs.iter().enumerate() {
            if i < args.len() {
                let raw = backend.emit_expr(out, &args[i], indent);
                // Phase 3: Decay chimera arguments before FFI call
                let raw = backend.emit_decay(out, &raw, Some(arg_ty), indent);
                match arg_ty {
                    Type::Custom(__t) if __t == "Int" || __t == "UInt" => marshaled.push(format!("i64 {}", raw)),
                    Type::Custom(__t) if __t == "Bool" => {
                        let boxed = backend.adapt_to_i64(out, indent, &raw);
                        let z = format!("%fz{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, z, boxed).ok();
                        marshaled.push(format!("i32 {}", z));
                    }
                    Type::Custom(__t) if __t == "Char" => {
                        let boxed = backend.adapt_to_i64(out, indent, &raw);
                        let z = format!("%fz{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, z, boxed).ok();
                        marshaled.push(format!("i32 {}", z));
                    }
                    Type::Custom(__t) if __t == "Float" => {
                        let fl = backend.ensure_float_reg(out, indent, &raw);
                        marshaled.push(format!("float {}", fl));
                    }
                    Type::Custom(__t) if __t == "String" || __t == "Data" => {
                        let boxed = backend.adapt_to_i64(out, indent, &raw);
                        let p = format!("%fp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, boxed).ok();
                        marshaled.push(format!("ptr {}", p));
                    }
                    _ => marshaled.push(format!("i64 {}", raw)),
                }
            }
        }
        // Generic FFI call — no special-case magic
        let is_float_ret = match &ret_type {
            crate::ast::ResultType::Projection(ts) => ts.iter().any(|t| matches!(t, Type::Custom(__t) if __t == "Float")),
            _ => false,
        };
        let call_ret = if is_float_ret { "float" } else { "i64" };
        let args_str = marshaled.join(", ");
        // 2026-06-28: Use txn_counter to prevent %t{N} collision
        let call_result = format!("%t{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = call {} @{}({})", indent, call_result, call_ret, name, args_str).ok();

        // Pipe-syntax frgn: emit sentinel checks using select (branchless).
        // String/Data: null pointer → use fallback
        // Float: NaN/Inf → use fallback
        // Int/UInt/Bool/Char: always valid (no sentinel needed)
        if is_pipe {
            let success_ty = success_output.first()
                .map(|(_, t)| t)
                .cloned()
                .unwrap_or(Type::Void);
            let fallback_reg = fallback.as_ref().map(|e| backend.emit_expr(out, e, indent));

            match (&success_ty, is_float_ret) {
                (Type::Custom(__t), _) if __t == "String" || __t == "Data" => {
                    // Null pointer check for i8* returns
                    let is_null = format!("%pipe_null{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    // call_result is i64 (boxed ptr). Convert to ptr for null check.
                    let ptr = format!("%pipe_ptr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, call_result).ok();
                    writeln!(out, "{}{} = icmp eq ptr {}, null", indent, is_null, ptr).ok();
                    // 2026-06-28: Use txn_counter to prevent %t{N} collision
                    let select_reg = format!("%t{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    let fbr = fallback_reg.as_ref().map(|r| r.name.as_str()).unwrap_or("null");
                    writeln!(out, "{}{} = select i1 {}, i64 {}, i64 {}",
                        indent, select_reg, is_null, fbr, call_result).ok();
                    return TypedRegister { name: select_reg, ty: Type::Custom("Int".to_string()) };
                }
                (Type::Custom(__t), _) if __t == "Float" => {
                    // NaN check for float returns
                    let is_nan = format!("%pipe_nan{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = fcmp uno float {}, {}", indent, is_nan, call_result, call_result).ok();
                    // 2026-06-28: Use txn_counter to prevent %t{N} collision
                    let select_reg = format!("%t{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    let fbr = fallback_reg.as_ref().map(|r| r.name.as_str()).unwrap_or("0.0");
                    writeln!(out, "{}{} = select i1 {}, float {}, float {}",
                        indent, select_reg, is_nan, fbr, call_result).ok();
                    let bi = format!("%fbi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    let ze = format!("%fze{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, select_reg).ok();
                    writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                    backend.fun.reg_float_cache.insert(ze.clone(), select_reg.clone());
                    return TypedRegister { name: ze, ty: Type::Custom("Float".to_string()) };
                }
                _ => {
                    // Int/UInt/Bool/Char: always valid, just pass through
                    if is_float_ret {
                        let bi = format!("%fbi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        let ze = format!("%fze{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, call_result).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                        backend.fun.reg_float_cache.insert(ze.clone(), call_result.clone());
                        return TypedRegister { name: ze, ty: Type::Custom("Float".to_string()) };
                    }
                    return TypedRegister { name: call_result, ty: Type::Custom("Int".to_string()) };
                }
            }
        }

        if is_float_ret {
            let bi = format!("%fbi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ze = format!("%fze{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, call_result).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
            backend.fun.reg_float_cache.insert(ze.clone(), call_result.clone());
            return TypedRegister { name: ze, ty: Type::Custom("Float".to_string()) };
        }
        return TypedRegister { name: call_result, ty: Type::Custom("Int".to_string()) };
    } else {
        // Internal call — marshal i64 back to real types per definition
        let def_tys: Option<Vec<Type>> = backend.ctx.defn_params.get(name).cloned();
        let def_rets: Option<Vec<Type>> = backend.ctx.defn_return_types.get(name).cloned();
        let mut a_strs = Vec::new();
        for (ai, arg) in args.iter().enumerate() {
            let raw = backend.emit_expr(out, arg, indent);
            if let Some(ref tys) = def_tys {
                if ai < tys.len() {
                    match &tys[ai] {
                        Type::Custom(__t) if __t == "Bool" => {
                            let boxed = backend.adapt_to_i64(out, indent, &raw);
                            let tr = format!("%ctr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, boxed).ok();
                            a_strs.push(format!("i8 {}", tr));
                        }
                        Type::Custom(__t) if __t == "String" || __t == "Data" => {
                            let boxed = backend.adapt_to_i64(out, indent, &raw);
                            let p = format!("%cip{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, boxed).ok();
                            a_strs.push(format!("ptr {}", p));
                        }
                        Type::Custom(__t) if __t == "Float" => {
                            let fl = backend.ensure_float_reg(out, indent, &raw);
                            a_strs.push(format!("float {}", fl));
                        }
                        _ => a_strs.push(format!("i64 {}", raw)),
                    }
                } else {
                    a_strs.push(format!("i64 {}", raw));
                }
            } else {
                // 2026-06-17: zext Bool/Char/Float to i64 for enum variant storage
                let stored = if raw.ty == Type::Custom("Bool".to_string()) {
                    let z = format!("%cz{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = zext i1 {} to i64", indent, z, raw.name).ok();
                    z
                } else if raw.ty == Type::Custom("Char".to_string()) {
                    // Char registers are already i64 from emit_expr
                    raw.name.clone()
                } else if raw.ty == Type::Custom("Float".to_string()) {
                    let bi = format!("%cfb{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, raw.name).ok();
                    let ze = format!("%cfz{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                    ze
                } else {
                    raw.name.clone()
                };
                a_strs.push(format!("i64 {}", stored));
            }
        }
        if name.starts_with(|c: char| c.is_uppercase()) && !backend.program_txns.contains(&name.to_string()) {
            let disc_val = backend.ctx.variant_disc.get(name)
                .map(|(_, d, _)| *d)
                .unwrap_or(0u64);
            let n_slots = a_strs.len() + 1;
            let sz = format!("%csz{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = mul i64 {}, 8", indent, sz, n_slots as i64).ok();
            // Why malloc/arena for enum variants: tagged union requires heap
            // allocation because different variants have different sizes.
            // Arena handles this with bump alloc when in a loop context.
            let pm = backend.emit_arena_alloc(out, indent, &sz);
            let p = format!("%cop{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, p, pm).ok();
            let disc_gep = format!("%cdg{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 0", indent, disc_gep, p).ok();
            writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, disc_val, disc_gep).ok();
            for (ai, arg_reg) in a_strs.iter().enumerate() {
                let pay_gep = format!("%cpg{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let parts: Vec<&str> = arg_reg.splitn(2, ' ').collect();
                let rn = if parts.len() == 2 { parts[1] } else { arg_reg };
                writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 {}", indent, pay_gep, p, ai + 1).ok();
                // 2026-06-17: Box float to i64 for enum storage
                if parts.len() == 2 && (parts[0] == "float" || parts[0] == "float,") {
                    let bi = format!("%fbe{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, rn).ok();
                    let ze = format!("%fze{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                    writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, ze, pay_gep).ok();
                } else {
                    eprintln!("DBG_store: arg_reg={:?}, parts={:?}, rn={:?}", arg_reg, parts, rn);
                    writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, rn, pay_gep).ok();
                }
            }
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, p).ok();
            return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
        } else {
            // 2026-06-13: Pass %state to defns/callable txns — functions need
            // the state pointer to access module-level fields (SSA is function-scoped).
            let fn_name = if name == "main" && backend.ctx.defn_params.contains_key("main") {
                "brief_main"
            } else {
                name
            };
            a_strs.insert(0, "ptr %state".to_string());
            let is_float_ret = def_rets.as_ref().map_or(false, |rets| rets.iter().any(|t| matches!(t, Type::Custom(__t) if __t == "Float")));
            let call_ret = if is_float_ret { "float" } else { "i64" };
            writeln!(out, "{}{} = call {} @{}({})", indent, v, call_ret, fn_name, a_strs.join(", ")).ok();
            if is_float_ret {
                return TypedRegister { name: v.to_string(), ty: Type::Custom("Float".to_string()) };
            }
            // Internal calls return i64 (boxed), so mark as Type::Custom("Int".to_string()).
            return TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) };
        }
    }
}

// 2026-07-03: Try to emit an indirect call through a function pointer variable.
// Returns Some if name is a local variable of fn-pointer type, None otherwise.
fn try_fn_ptr_call(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    name: &str,
    args: &[Expr],
    indent: &str,
) -> Option<TypedRegister> {
    // Clone upfront to avoid borrow conflicts with emit_expr
    let var_ty = backend.fun.let_binding_types.get(name)?.clone();
    let Type::Applied(fn_name, inner) = &var_ty else { return None; };
    if fn_name != "Fn" || inner.len() != 2 {
        return None;
    }
    let (param_types, ret_type) = (inner[0].clone(), inner[1].clone());
    let fn_reg = backend.fun.let_bindings.get(name)
        .cloned()
        .unwrap_or_else(|| "0".to_string());
    // 2026-07-10: If the variable type is Ptr<Fn> or PtrConst<Fn>, the register
    // is already a ptr (from &fn or Deref). Otherwise, it's an i64 (from
    // ptrtoint via :> Ptr or let-binding of fn type) and needs inttoptr.
    let is_fn_ptr = backend.fun.let_binding_types.get(name)
        .and_then(|t| crate::type_universe::pointee_type(t))
        .is_some();
    let fn_ptr = if is_fn_ptr {
        fn_reg
    } else {
        let p = format!("%ic_ptr{}", backend.fun.txn_counter);
        backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, fn_reg).ok();
        p
    };
    // Marshal arguments matching internal call convention: %state + typed args
    let mut arg_strs: Vec<String> = Vec::new();
    arg_strs.push("ptr %state".to_string());
    let Type::Tuple(params) = &param_types else {
        // Single return — no params; just pass %state
        let ret = emit_indirect_return(backend, out, v, indent, &fn_ptr, &arg_strs, &ret_type);
        return Some(ret);
    };
    for (i, arg) in args.iter().enumerate() {
        let val = backend.emit_expr(out, arg, indent);
        let expected = params.get(i).cloned().unwrap_or(Type::Custom("Int".to_string()));
        match &expected {
            Type::Custom(__t) if __t == "Bool" => {
                let boxed = backend.adapt_to_i64(out, indent, &val);
                let tr = format!("%ic_tr{}", backend.fun.txn_counter);
                backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, boxed).ok();
                arg_strs.push(format!("i8 {}", tr));
            }
            Type::Custom(__t) if __t == "Float" => {
                let fl = backend.ensure_float_reg(out, indent, &val);
                arg_strs.push(format!("float {}", fl));
            }
            Type::Custom(__t) if __t == "String" || __t == "Data" => {
                let boxed = backend.adapt_to_i64(out, indent, &val);
                let p = format!("%ic_p{}", backend.fun.txn_counter);
                backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, boxed).ok();
                arg_strs.push(format!("ptr {}", p));
            }
            _ => arg_strs.push(format!("i64 {}", val)),
        }
    }
    let ret = emit_indirect_return(backend, out, v, indent, &fn_ptr, &arg_strs, &ret_type);
    Some(ret)
}

// Emit the indirect call and handle return type marshalling.
fn emit_indirect_return(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    indent: &str,
    fn_ptr: &str,
    arg_strs: &[String],
    ret_type: &Type,
) -> TypedRegister {
    let is_float = matches!(ret_type, Type::Custom(__t) if __t == "Float");
    let call_ret = if is_float { "float" } else { "i64" };
    let call_result = format!("%ic_res{}", backend.fun.txn_counter);
    backend.fun.txn_counter += 1;
    writeln!(out, "{}{} = call {} {}({})", indent, call_result, call_ret, fn_ptr, arg_strs.join(", ")).ok();
    if is_float {
        let bi = format!("%ic_bi{}", backend.fun.txn_counter);
        let ze = format!("%ic_ze{}", backend.fun.txn_counter);
        backend.fun.txn_counter += 1;
        writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, call_result).ok();
        writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
        backend.fun.reg_float_cache.insert(ze.clone(), call_result);
        TypedRegister { name: ze, ty: Type::Custom("Float".to_string()) }
    } else {
        writeln!(out, "{}{} = add i64 0, {}", indent, v, call_result).ok();
        TypedRegister { name: v.to_string(), ty: Type::Custom("Int".to_string()) }
    }
}
