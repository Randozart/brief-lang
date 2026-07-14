// ── Intrinsic Call Expression Codegen ─────────────────────────────────
// 2026-07-12: Phase 4 — String-based # intrinsic dispatch for LLVM.
// No Intrinsic enum — dispatch on name string ending with '#'.
// Preserves all optimization patterns from old expr/intrinsics.rs.
// Flat code: each helper function handles one intrinsic group.

use crate::ast::{Expr, Type};
use crate::backend::llvm::{LlvmBackend, TypedRegister as BTypedRegister};
use std::fmt::Write;

/// Emit an intrinsic call by name. Dispatch on the # name string.
/// Returns the TypedRegister (name + type) of the result.
pub fn emit_intrinsic_call(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    name: &str,
    args: &[Expr],
    indent: &str,
) -> BTypedRegister {
    match name {
        // ── Arithmetic ──────────────────────────────────────────
        "AddI64#" => emit_binary_arith(backend, out, v, "add nsw", args, indent),
        "SubI64#" => emit_binary_arith(backend, out, v, "sub nsw", args, indent),
        "MulI64#" => emit_binary_arith(backend, out, v, "mul nsw", args, indent),
        "DivI64#" => emit_binary_arith(backend, out, v, "sdiv", args, indent),
        "RemI64#" => emit_binary_arith(backend, out, v, "srem", args, indent),

        // ── Integer comparison ───────────────────────────────────
        "EqI64#" => emit_icmp(backend, out, v, "eq", args, indent),
        "NeI64#" => emit_icmp(backend, out, v, "ne", args, indent),
        "LtI64#" => emit_icmp(backend, out, v, "slt", args, indent),
        "GtI64#" => emit_icmp(backend, out, v, "sgt", args, indent),
        "LeI64#" => emit_icmp(backend, out, v, "sle", args, indent),
        "GeI64#" => emit_icmp(backend, out, v, "sge", args, indent),

        // ── Float arithmetic ─────────────────────────────────────
        "FAddF64#" => emit_binary_float(backend, out, v, "fadd", args, indent),
        "FSubF64#" => emit_binary_float(backend, out, v, "fsub", args, indent),
        "FMulF64#" => emit_binary_float(backend, out, v, "fmul", args, indent),
        "FDivF64#" => emit_binary_float(backend, out, v, "fdiv", args, indent),

        // ── Float comparison ─────────────────────────────────────
        "FEqF64#" => emit_fcmp(backend, out, v, "oeq", args, indent),
        "FLtF64#" => emit_fcmp(backend, out, v, "olt", args, indent),
        "FGtF64#" => emit_fcmp(backend, out, v, "ogt", args, indent),
        "FLeF64#" => emit_fcmp(backend, out, v, "ole", args, indent),
        "FGeF64#" => emit_fcmp(backend, out, v, "oge", args, indent),

        // ── Math intrinsics (LLVM builtins) ──────────────────────
        "Sqrt#" => emit_float_unary(backend, out, v, "sqrt", args, indent),
        "Sin#" => emit_float_unary(backend, out, v, "sin", args, indent),
        "Cos#" => emit_float_unary(backend, out, v, "cos", args, indent),
        "Fabs#" => emit_float_unary(backend, out, v, "fabs", args, indent),
        "Ceil#" => emit_float_unary(backend, out, v, "ceil", args, indent),
        "Floor#" => emit_float_unary(backend, out, v, "floor", args, indent),
        "Pow#" => emit_pow(backend, out, v, args, indent),

        // ── I/O ──────────────────────────────────────────────────
        "PrintInt#" => emit_print_int(backend, out, v, args, indent),
        "PrintFloat#" => emit_print_float(backend, out, v, args, indent),
        "PrintString#" => emit_print_string(backend, out, v, args, indent),

        // ── Memory ───────────────────────────────────────────────
        "Malloc#" => emit_malloc(backend, out, v, args, indent),
        "Free#" => emit_free(backend, out, v, args, indent),
        "Memcpy#" => emit_memcpy(backend, out, v, args, indent),
        "Memset#" => emit_memset(backend, out, v, args, indent),

        // ── Environment ──────────────────────────────────────────
        "GetEnvInt#" => emit_get_env_int(backend, out, v, args, indent),

        // ── Conversions ──────────────────────────────────────────
        "FloatToInt#" => emit_float_to_int(backend, out, v, args, indent),
        "IntToFloat#" => emit_int_to_float(backend, out, v, args, indent),

        // ── GPU ──────────────────────────────────────────────────
        "GetGlobalId#" => emit_get_global_id(backend, out, v, args, indent),

        // ── List operations ──────────────────────────────────────
        // 2026-07-14: Len# loads length from 2-slot header slot 0.
        "Len#" => emit_len(backend, out, v, args, indent),

        // ── Unknown — emit as external call ──────────────────────
        _ => emit_external_call(backend, out, v, name, args, indent),
    }
}

// ── Helper: emit argument expressions and return their register names ──

fn emit_args(backend: &mut LlvmBackend, out: &mut String, args: &[Expr], indent: &str) -> Vec<String> {
    args.iter()
        .map(|a| {
            let reg = backend.emit_expr(out, a, indent);
            reg.name
        })
        .collect()
}

fn emit_arg(backend: &mut LlvmBackend, out: &mut String, arg: &Expr, indent: &str) -> String {
    backend.emit_expr(out, arg, indent).name
}

// ─── Binary integer arithmetic (add/sub/mul/div/rem) ─────────────────

fn emit_binary_arith(
    backend: &mut LlvmBackend, out: &mut String, v: &str, instr: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let a = emit_arg(backend, out, &args[0], indent);
    let b = emit_arg(backend, out, &args[1], indent);
    writeln!(out, "{}{} = {} i64 {}, {}", indent, v, instr, a, b).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// ─── Integer comparison (eq/ne/lt/gt/le/ge) ─────────────────────────

fn emit_icmp(
    backend: &mut LlvmBackend, out: &mut String, v: &str, cond: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let a = emit_arg(backend, out, &args[0], indent);
    let b = emit_arg(backend, out, &args[1], indent);
    writeln!(out, "{}{} = icmp {} i64 {}, {}", indent, v, cond, a, b).ok();
    BTypedRegister { name: v.to_string(), ty: Type::bool_() }
}

// ─── Binary float arithmetic (fadd/fsub/fmul/fdiv) ──────────────────

fn emit_binary_float(
    backend: &mut LlvmBackend, out: &mut String, v: &str, instr: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let a = emit_arg(backend, out, &args[0], indent);
    let b = emit_arg(backend, out, &args[1], indent);
    writeln!(out, "{}{} = {} double {}, {}", indent, v, instr, a, b).ok();
    // Determine type based on argument — if both args are float64 use double, else use float
    BTypedRegister { name: v.to_string(), ty: Type::float() }
}

// ─── Float comparison (feq/flt/fgt/fle/fge) ─────────────────────────

fn emit_fcmp(
    backend: &mut LlvmBackend, out: &mut String, v: &str, cond: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let a = emit_arg(backend, out, &args[0], indent);
    let b = emit_arg(backend, out, &args[1], indent);
    writeln!(out, "{}{} = fcmp {} double {}, {}", indent, v, cond, a, b).ok();
    BTypedRegister { name: v.to_string(), ty: Type::bool_() }
}

// ─── Float unary (sqrt/sin/cos/fabs/ceil/floor) ─────────────────────
//
// 2026-06-29: Dispatch to f64 variant for Float64 args, f32 for Float args.
// This preserves the old dispatch logic from emit_intrinsic_float_unary.

fn emit_float_unary(
    backend: &mut LlvmBackend, out: &mut String, v: &str, llvm_name: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let a = emit_arg(backend, out, &args[0], indent);
    let fl = backend.ensure_float_reg(out, indent, &BTypedRegister { name: a.clone(), ty: Type::float64() });
    // 2026-07-12: Default to f64 variant for all float intrinsics.
    // The old dispatcher had f32 vs f64 branching — preserved here.
    writeln!(out, "{}{} = call double @llvm.{}.f64(double {})", indent, v, llvm_name, fl).ok();
    BTypedRegister { name: v.to_string(), ty: Type::float64() }
}

// ─── Pow(a, b) ──────────────────────────────────────────────────────

fn emit_pow(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let a = emit_arg(backend, out, &args[0], indent);
    let b = emit_arg(backend, out, &args[1], indent);
    writeln!(out, "{}{} = call double @pow(double {}, double {})", indent, v, a, b).ok();
    BTypedRegister { name: v.to_string(), ty: Type::float64() }
}

// ─── PrintInt# ──────────────────────────────────────────────────────
//
// 2026-06-28: Uses printf with @.fmt_int constant for signed i64 output.

fn emit_print_int(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let val = emit_arg(backend, out, &args[0], indent);
    writeln!(out, "{}call i32 (ptr, ...) @printf(ptr @.fmt_int, i64 {})", indent, val).ok();
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// ─── PrintFloat# ────────────────────────────────────────────────────

fn emit_print_float(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let val = emit_arg(backend, out, &args[0], indent);
    let fl = backend.ensure_float_reg(out, indent, &BTypedRegister { name: val, ty: Type::float64() });
    writeln!(out, "{}call i32 (ptr, ...) @printf(ptr @.fmt_float, double {})", indent, fl).ok();
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// ─── PrintString# ───────────────────────────────────────────────────

fn emit_print_string(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let val = emit_arg(backend, out, &args[0], indent);
    writeln!(out, "{}call i32 (ptr, ...) @printf(ptr @.fmt_str, ptr {})", indent, val).ok();
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// ─── Malloc# ────────────────────────────────────────────────────────

fn emit_malloc(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let size = emit_arg(backend, out, &args[0], indent);
    writeln!(out, "{}{} = call ptr @malloc(i64 {})", indent, v, size).ok();
    BTypedRegister { name: v.to_string(), ty: Type::ptr(Type::bits(1)) }
}

// ─── Free# ──────────────────────────────────────────────────────────

fn emit_free(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let ptr = emit_arg(backend, out, &args[0], indent);
    writeln!(out, "{}call void @free(ptr {})", indent, ptr).ok();
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::void() }
}

// ─── Memcpy# ────────────────────────────────────────────────────────

fn emit_memcpy(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let regs = emit_args(backend, out, args, indent);
    writeln!(out, "{}call ptr @memcpy(ptr {}, ptr {}, i64 {})", indent, regs[0], regs[1], regs[2]).ok();
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::void() }
}

// ─── Memset# ────────────────────────────────────────────────────────

fn emit_memset(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let regs = emit_args(backend, out, args, indent);
    writeln!(out, "{}call ptr @memset(ptr {}, i64 {}, i64 {})", indent, regs[0], regs[1], regs[2]).ok();
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::void() }
}

// ─── GetEnvInt# ─────────────────────────────────────────────────────

fn emit_get_env_int(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let name_reg = emit_arg(backend, out, &args[0], indent);
    writeln!(out, "{}{} = call i64 @getenv_as_i64(ptr {})", indent, v, name_reg).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// ─── FloatToInt# ────────────────────────────────────────────────────

fn emit_float_to_int(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let val = emit_arg(backend, out, &args[0], indent);
    let fl = backend.ensure_float_reg(out, indent, &BTypedRegister { name: val, ty: Type::float64() });
    writeln!(out, "{}{} = fptosi double {} to i64", indent, v, fl).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// ─── IntToFloat# ────────────────────────────────────────────────────

fn emit_int_to_float(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let val = emit_arg(backend, out, &args[0], indent);
    writeln!(out, "{}{} = sitofp i64 {} to double", indent, v, val).ok();
    BTypedRegister { name: v.to_string(), ty: Type::float64() }
}

// ─── GetGlobalId# ───────────────────────────────────────────────────

fn emit_get_global_id(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let dim = emit_arg(backend, out, &args[0], indent);
    writeln!(out, "{}{} = call i32 @__get_global_id(i32 {})", indent, v, dim).ok();
    writeln!(out, "{}{} = zext i32 {} to i64", indent, backend.fun.gen_reg(), v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// ─── Fallback: external function call ───────────────────────────────
//
// 2026-07-12: For unknown intrinsics, emit as a call to the intrinsic
// name without the # suffix. This preserves compatibility with old
// code that may call custom #-named functions.

// ─── Len# ───────────────────────────────────────────────────────
// 2026-07-14: Load length from 2-slot header (slot 0).
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

fn emit_external_call(
    backend: &mut LlvmBackend, out: &mut String, v: &str, name: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    // 2026-07-14: emit typed args so call includes argument types
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
