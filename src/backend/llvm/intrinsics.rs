// ── Intrinsic Call Expression Codegen ─────────────────────────────────
// 2026-07-14: Config-driven operation dispatch. Uses config/llvm-ops.toml
// for generic operations (Add#, Eq#, etc.) and special-case helpers for
// memory/I/O intrinsics (Malloc#, Print#, etc.) that don't fit templates.
// Flat code: max 2 nesting depth.

use std::sync::LazyLock;
use crate::ast::{Expr, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister as BTypedRegister};
use crate::config::{OpConfig, derive_llvm_type, TypeConfig};
use std::fmt::Write;

static OP_CONFIG: LazyLock<OpConfig> = LazyLock::new(|| OpConfig::load());
static TYPE_CONFIG: LazyLock<TypeConfig> = LazyLock::new(|| TypeConfig::load());

/// Emit an intrinsic call by name. For generic operations (Add#, Eq#, etc.)
/// looks up the IR template from config/llvm-ops.toml using (op, primitive, bytes)
/// of the first argument. For memory/I/O intrinsics, uses special-case helpers.
pub fn emit_intrinsic_call(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    name: &str,
    args: &[Expr],
    indent: &str,
) -> BTypedRegister {
    // Special-case intrinsics that don't fit the template pattern
    match name {
        "Malloc#" => return emit_malloc(backend, out, v, args, indent),
        "Free#" => return emit_free(backend, out, v, args, indent),
        "Memcpy#" => return emit_memcpy(backend, out, v, args, indent),
        "Memset#" => return emit_memset(backend, out, v, args, indent),
        "Print#" => return emit_print(backend, out, v, args, indent),
        "GetEnv#" => return emit_get_env(backend, out, v, args, indent),
        "GetGlobalId#" => return emit_get_global_id(backend, out, v, args, indent),
        "Len#" | "Length#" => return emit_len(backend, out, v, args, indent),
        "Concat#" => return emit_external_call(backend, out, v, name, args, indent),
        "Length#" => return emit_external_call(backend, out, v, name, args, indent),
        "Get#" => return emit_external_call(backend, out, v, name, args, indent),
        "Insert#" => return emit_external_call(backend, out, v, name, args, indent),
        _ => {}
    }

    // For generic operations, look up template from config
    let arg_regs: Vec<BTypedRegister> = args.iter()
        .map(|a| backend.emit_expr(out, a, indent))
        .collect();

    if arg_regs.is_empty() {
        return BTypedRegister { name: v.to_string(), ty: Type::void() };
    }

    // Determine primitive and bytes from the first argument's type
    let prim = resolve_arg_primitive(backend, &arg_regs[0]);
    let bytes = resolve_arg_bytes(backend, &arg_regs[0]).unwrap_or(8);

    let op_name = name.trim_end_matches('#');
    if let Some(template) = OP_CONFIG.lookup(op_name, &prim, bytes) {
        let ir = template
            .replace("%v", v)
            .replace("%a", &arg_regs.get(0).map(|r| r.name.clone()).unwrap_or_default())
            .replace("%b", &arg_regs.get(1).map(|r| r.name.clone()).unwrap_or_default())
            .replace("%c", &arg_regs.get(2).map(|r| r.name.clone()).unwrap_or_default());
        writeln!(out, "{}  {}", indent, ir).ok();
        // Try to determine return type from the template or fall back to i64
        return BTypedRegister { name: v.to_string(), ty: Type::int() };
    }

    // Fallback: emit as external call
    emit_external_call(backend, out, v, name, args, indent)
}

/// Resolve the primitive metadata for a typed register's type.
fn resolve_arg_primitive(backend: &LlvmBackend, reg: &BTypedRegister) -> String {
    backend.ctx.type_universe.as_ref()
        .and_then(|u| crate::type_universe::resolve_type(u, &reg.ty))
        .and_then(|rt| rt.primitive().map(|s| s.to_string()))
        .unwrap_or_else(|| "Int".to_string())
}

fn resolve_arg_bytes(backend: &LlvmBackend, reg: &BTypedRegister) -> Option<u64> {
    backend.ctx.type_universe.as_ref()
        .and_then(|u| crate::type_universe::resolve_type(u, &reg.ty))
        .map(|rt| rt.bytes)
}

// ── Helper: emit argument expressions and return their register names ──

fn emit_args(backend: &mut LlvmBackend, out: &mut String, args: &[Expr], indent: &str) -> Vec<String> {
    args.iter()
        .map(|a| backend.emit_expr(out, a, indent).name)
        .collect()
}

fn emit_arg(backend: &mut LlvmBackend, out: &mut String, arg: &Expr, indent: &str) -> String {
    backend.emit_expr(out, arg, indent).name
}

// ─── Print# (polymorphic — prints int, float, or string) ──────────────

fn emit_print(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let reg = backend.emit_expr(out, &args[0], indent);
    match resolve_arg_primitive(backend, &reg).as_str() {
        "Float" => {
            let fl = backend.ensure_float_reg(out, indent, &reg);
            writeln!(out, "{}call i32 (ptr, ...) @printf(ptr @FMT_FLOAT, double {})", indent, fl).ok();
        }
        "String" => {
            writeln!(out, "{}call i32 (ptr, ...) @printf(ptr @FMT_STR, ptr {})", indent, reg.name).ok();
        }
        _ => {
            writeln!(out, "{}call i32 (ptr, ...) @printf(ptr @FMT_INT, i64 {})", indent, reg.name).ok();
        }
    }
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// ─── Memory intrinsics ────────────────────────────────────────────────

fn emit_malloc(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let size = emit_arg(backend, out, &args[0], indent);
    writeln!(out, "{}{} = call ptr @malloc(i64 {})", indent, v, size).ok();
    BTypedRegister { name: v.to_string(), ty: Type::ptr(Type::bits(1)) }
}

fn emit_free(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let ptr = emit_arg(backend, out, &args[0], indent);
    writeln!(out, "{}call void @free(ptr {})", indent, ptr).ok();
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::void() }
}

fn emit_memcpy(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let regs = emit_args(backend, out, args, indent);
    writeln!(out, "{}call ptr @memcpy(ptr {}, ptr {}, i64 {})", indent, regs[0], regs[1], regs[2]).ok();
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::void() }
}

fn emit_memset(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let regs = emit_args(backend, out, args, indent);
    writeln!(out, "{}call ptr @memset(ptr {}, i64 {}, i64 {})", indent, regs[0], regs[1], regs[2]).ok();
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::void() }
}

// ─── GetEnv# ──────────────────────────────────────────────────────────

fn emit_get_env(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let name_reg = emit_arg(backend, out, &args[0], indent);
    let env_ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = call ptr @getenv(ptr {})", indent, env_ptr, name_reg).ok();
    writeln!(out, "{}{} = call i64 @atol(ptr {})", indent, v, env_ptr).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// ─── GetGlobalId# ─────────────────────────────────────────────────────

fn emit_get_global_id(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let dim = emit_arg(backend, out, &args[0], indent);
    writeln!(out, "{}{} = call i32 @__get_global_id(i32 {})", indent, v, dim).ok();
    let ext = backend.fun.gen_reg();
    writeln!(out, "{}{} = zext i32 {} to i64", indent, ext, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// ─── Len# / Length# — load list length from 2-slot header ──────────────

fn emit_len(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let list = emit_arg(backend, out, &args[0], indent);
    let ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, list).ok();
    writeln!(out, "{}{} = load i64, ptr {}", indent, v, ptr).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// ─── External call fallback ──────────────────────────────────────────

fn emit_external_call(
    backend: &mut LlvmBackend, out: &mut String, v: &str, name: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let typed_regs: Vec<BTypedRegister> = args.iter()
        .map(|a| backend.emit_expr(out, a, indent))
        .collect();
    let arg_strs: Vec<String> = typed_regs.iter()
        .map(|reg| format!("{} {}", crate::backend::llvm::types::lower_type(&reg.ty), reg.name))
        .collect();
    let clean_name = name.trim_end_matches('#');
    writeln!(out, "{}{} = call i64 @{}({})", indent, v, clean_name, arg_strs.join(", ")).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}
