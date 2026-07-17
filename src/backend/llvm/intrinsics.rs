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
        "Load#" => return emit_load(backend, out, v, args, indent),
        "Store#" => return emit_store(backend, out, v, args, indent),
        "Copy#" => return emit_copy(backend, out, v, args, indent),
        "Fill#" => return emit_fill(backend, out, v, args, indent),
        "Print#" => return emit_print(backend, out, v, args, indent),
        "GetEnv#" => return emit_get_env(backend, out, v, args, indent),
        "GetGlobalId#" => return emit_get_global_id(backend, out, v, args, indent),
        "GetGlobalSize#" => return emit_external_call(backend, out, v, name, args, indent),
        "GetLocalId#" => return emit_external_call(backend, out, v, name, args, indent),
        "AddressOf#" => return emit_address_of(backend, out, v, args, indent),
        "SysCall#" => return emit_syscall(backend, out, v, args, indent),
        "SysConf#" => return emit_sysconf(backend, out, v, args, indent),
        "Len#" | "Length#" => return emit_len(backend, out, v, args, indent),
        "Concat#" => return emit_external_call(backend, out, v, name, args, indent),
        "Length#" => return emit_external_call(backend, out, v, name, args, indent),
        "Get#" => return emit_external_call(backend, out, v, name, args, indent),
        "Insert#" => return emit_external_call(backend, out, v, name, args, indent),
        // 2026-07-15: Atomic operations (LLVM atomic instructions)
        "AtomicLoad#" => return emit_atomic_load(backend, out, v, args, indent),
        "AtomicStore#" => return emit_atomic_store(backend, out, v, args, indent),
        "AtomicCas#" => return emit_atomic_cas(backend, out, v, args, indent),
        "AtomicXchg#" => return emit_atomic_xchg(backend, out, v, args, indent),
        "AtomicAdd#" => return emit_atomic_add(backend, out, v, args, indent),
        "Fence#" => return emit_fence(backend, out, v, args, indent),
        // 2026-07-15: Dynamic linker intrinsics
        "DlOpen#" => return emit_dl_open(backend, out, v, args, indent),
        "DlSym#" => return emit_dl_sym(backend, out, v, args, indent),
        "DlClose#" => return emit_dl_close(backend, out, v, args, indent),
        // 2026-07-15: Debugging intrinsics
        "Backtrace#" => return emit_backtrace(backend, out, v, args, indent),
        _ => {}
    }

    // For generic operations, look up template from config
    let arg_regs: Vec<BTypedRegister> = args.iter()
        .map(|a| backend.emit_expr(out, a, indent))
        .collect();

    if arg_regs.is_empty() {
        return BTypedRegister { name: v.to_string(), ty: Type::void() };
    }

    // Determine ctd and bytes from the first argument's type
    // 2026-07-17: CTD replaces primitive — ops TOML uses CTD-compatible keys
    let prim = resolve_arg_ctd(backend, &arg_regs[0]);
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

/// Resolve the CTD metadata for a typed register's type.
/// 2026-07-17: Replaced primitive() with CTD property read.
fn resolve_arg_ctd(backend: &LlvmBackend, reg: &BTypedRegister) -> String {
    backend.ctx.type_universe.as_ref()
        .and_then(|u| crate::type_universe::resolve_type(u, &reg.ty))
        .and_then(|rt| rt.properties.get("ctd").and_then(|pv| {
            if let crate::ast::PropertyValue::Identifier(s) = pv { Some(s.clone()) } else { None }
        }))
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
    // 2026-07-17: CTD replaces primitive — Float and String CTDs match directly.
    match resolve_arg_ctd(backend, &reg).as_str() {
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
    // 2026-07-17: Return Ptr<Int> so Expr::Index correctly identifies this
    // as a pointer type and emits GEP+load/store (not extractelement). The
    // raw bits are still i64 (ptrtoint); the type annotation only affects
    // downstream codegen dispatch. State storage boxes via adapt_to_i64.
    let name = v.trim_start_matches('%');
    writeln!(out, "{}%{}_p = call ptr @malloc(i64 {})", indent, name, size).ok();
    writeln!(out, "{}{} = ptrtoint ptr %{}_p to i64", indent, v, name).ok();
    BTypedRegister { name: v.to_string(), ty: Type::ptr(Type::int()) }
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

 fn emit_load(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let addr = emit_arg(backend, out, &args[0], indent);
    let ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr).ok();
    let bytes = args.get(1).and_then(|a| if let Expr::Decimal(n) = a { Some(*n as usize) } else { None }).unwrap_or(8);
    writeln!(out, "{}{} = load i{}, ptr {}", indent, v, bytes * 8, ptr).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

fn emit_store(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let addr = emit_arg(backend, out, &args[0], indent);
    let val = emit_arg(backend, out, &args[1], indent);
    let ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr).ok();
    let bytes = args.get(2).and_then(|a| if let Expr::Decimal(n) = a { Some(*n as usize) } else { None }).unwrap_or(8);
    writeln!(out, "{}store i{} {}, ptr {}", indent, bytes * 8, val, ptr).ok();
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::void() }
}

fn emit_copy(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let dst = emit_arg(backend, out, &args[0], indent);
    let src = emit_arg(backend, out, &args[1], indent);
    let len = emit_arg(backend, out, &args[2], indent);
    let dptr = backend.fun.gen_reg();
    let sptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, dptr, dst).ok();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sptr, src).ok();
    writeln!(out, "{}call void @llvm.memcpy.p0.p0.i64(ptr {}, ptr {}, i64 {}, i1 false)", indent, dptr, sptr, len).ok();
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::void() }
}

fn emit_fill(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let ptr_arg = emit_arg(backend, out, &args[0], indent);
    let val = emit_arg(backend, out, &args[1], indent);
    let len = emit_arg(backend, out, &args[2], indent);
    let p = backend.fun.gen_reg();
    let v8 = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, p, ptr_arg).ok();
    writeln!(out, "{}{} = trunc i64 {} to i8", indent, v8, val).ok();
    writeln!(out, "{}call void @llvm.memset.p0.i64(ptr {}, i8 {}, i64 {}, i1 false)", indent, p, v8, len).ok();
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::void() }
}

// ─── GetEnv# ──────────────────────────────────────────────────────────

fn emit_get_env(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let name_reg = emit_arg(backend, out, &args[0], indent);
    let ptr_reg = backend.fun.gen_reg();
    // 2026-07-15: name_reg is i64 — convert to ptr for C runtime
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr_reg, name_reg).ok();
    let env_ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = call ptr @getenv(ptr {})", indent, env_ptr, ptr_reg).ok();
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

// ─── AddressOf# — compile-time address resolution ─────────────────────

/// 2026-07-15: AddressOf# resolves a named device/entity to a typed pointer.
/// The address is resolved at compile time via the shared address_resolver
/// (which reads config/address-map.toml + hardcoded fallbacks).
/// Emits: %v = inttoptr i64 <addr> to ptr
fn emit_address_of(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    // Guard against empty args — address must be provided
    let Some(arg) = args.first() else {
        eprintln!("AddressOf#: warning — no arguments, emitting 0 as address");
        backend.emit_inttoptr(out, indent, &v, &"0");
        return BTypedRegister { name: v.to_string(), ty: Type::ptr(Type::bits(8)) };
    };
    // The argument must be a string literal at compile time
    let id = match arg {
        Expr::Quoted(bytes) => String::from_utf8_lossy(bytes).to_string(),
        _ => {
            // If not a literal, try emitting as expression and warn
            let reg = emit_arg(backend, out, &args[0], indent);
            eprintln!("AddressOf#: warning — argument is not a string literal, using runtime value");
            format!("dynamic_{}", reg)
        }
    };
    let addr = crate::address_resolver::resolve_address(&id);
    let addr_str = addr.to_string();
    backend.emit_inttoptr(out, indent, &v, &addr_str);
    BTypedRegister { name: v.to_string(), ty: Type::ptr(Type::bits(8)) }
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

// ─── SysCall# — raw OS syscall ───────────────────────────────────────

/// Resolve a PascalCase abstract op name to a syscall number (x86_64).
/// 2026-07-15: Single mapping table for all OS operations.
fn resolve_syscall_number(op: &str) -> Option<i64> {
    Some(match op {
        "Read" => 0, "Write" => 1, "Open" => 2, "Close" => 3,
        "Stat" => 4, "FStat" => 5, "LSeek" => 8, "Mmap" => 9,
        "Munmap" => 11, "Brk" => 12, "RtSigAction" => 13,
        "RtSigProcmask" => 14, "IoCtl" => 16, "Pipe" => 22,
        "SchedYield" => 24, "NanoSleep" => 35,
        "GetPid" => 39, "GetPPid" => 40, "Socket" => 41,
        "Connect" => 42, "Accept" => 43, "Send" => 44,
        "Recv" => 45, "SendTo" => 44, "RecvFrom" => 45,
        "Bind" => 49, "Listen" => 50, "Exit" => 60,
        "Fcntl" => 72, "FTruncate" => 77, "GetCwd" => 79,
        "ChDir" => 80, "MkDir" => 83, "RmDir" => 84,
        "Unlink" => 87, "Dup" => 32, "Dup2" => 33,
        "FSync" => 74, "MkDt" => 85, "ReadLink" => 89,
        "ChMod" => 90, "ChOwn" => 92, "UMask" => 95,
        "GetPgid" => 109, "GetSid" => 124, "ShmGet" => 29,
        "ShmAt" => 30, "ShmDt" => 31, "SemGet" => 64,
        "SemOp" => 65, "SemCtl" => 66, "ClockGetTime" => 228,
        "ClockSetTime" => 229, "Futex" => 202,
        "GetRandom" => 318, "Openat" => 257,
        "Membarrier" => 324, "CopyFileRange" => 326,
        "PRead" => 17, "PWrite" => 18,
        _ => return None,
    })
}

/// 2026-07-15: Emit SysCall# — first arg is op (Int raw number or PascalCase
/// abstract name), followed by up to 6 Int arguments.
/// Emits: call i64 @brief_syscall(i64 %num, i64 %a1, ..., i64 %a6)
fn emit_syscall(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    if args.is_empty() {
        writeln!(out, "{}call void @brief_syscall()", indent).ok();
        writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
        return BTypedRegister { name: v.to_string(), ty: Type::int() };
    }
    // Resolve the syscall number from the first argument
    let num_reg = match &args[0] {
        // Raw numeric syscall number: SysCall#(2, args...)
        Expr::Decimal(n) => format!("{}", n),
        // Abstract PascalCase op: SysCall#(Open, args...)
        Expr::Identifier(op) => {
            let n = resolve_syscall_number(op)
                .map(|n| n.to_string())
                .unwrap_or_else(|| {
                    eprintln!("SysCall#: unknown abstract op '{}', using 0", op);
                    "0".to_string()
                });
            n
        }
        _ => {
            // Fallback: emit as expression
            let reg = emit_arg(backend, out, &args[0], indent);
            reg
        }
    };
    // Emit remaining args as i64, padding to 7 total (num + 6 args)
    let mut all_args = vec![format!("i64 {}", num_reg)];
    for i in 1..args.len() {
        let reg = emit_arg(backend, out, &args[i], indent);
        all_args.push(format!("i64 {}", reg));
    }
    while all_args.len() < 7 {
        all_args.push("i64 0".to_string());
    }
    writeln!(out, "{}{} = call i64 @brief_syscall({})", indent, v, all_args.join(", ")).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// ─── SysConf# — runtime system configuration ──────────────────────────

/// 2026-07-15: Emit SysConf# — resolves POSIX sysconf() values at runtime.
/// First arg is a PascalCase abstract name (e.g., PageSize, CpuCount) or
/// a raw Int constant. Emits call to @brief_sysconf(i64 %name).
fn emit_sysconf(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let name_reg: String = match args.first() {
        Some(Expr::Identifier(name)) => {
            let n: i64 = match name.as_str() {
                "PageSize" => 30,
                "CpuCount" => 83,
                "HostNameMax" => 180,
                "OpenMax" => 4,
                "ArgMax" => 0,
                "ChildMax" => 1,
                "ClkTck" => 2,
                "NGroupsMax" => 3,
                _ => {
                    eprintln!("SysConf#: unknown abstract name '{}', using 0", name);
                    0
                }
            };
            n.to_string()
        }
        Some(Expr::Decimal(n)) => n.to_string(),
        Some(arg) => emit_arg(backend, out, arg, indent),
        None => "0".to_string(),
    };
    writeln!(out, "{}{} = call i64 @brief_sysconf(i64 {})", indent, v, name_reg).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// ─── Atomic operations ───────────────────────────────────────────────
// 2026-07-15: Each maps to a single LLVM atomic instruction.
// All use seq_cst ordering. The interpreter does non-atomic loads/stores
// (correct for single-threaded check mode).

fn emit_atomic_load(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let addr = emit_arg(backend, out, &args[0], indent);
    let ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr).ok();
    writeln!(out, "{}{} = load atomic i64, ptr {} seq_cst, align 8", indent, v, ptr).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

fn emit_atomic_store(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let addr = emit_arg(backend, out, &args[0], indent);
    let ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr).ok();
    let val = emit_arg(backend, out, &args[1], indent);
    // 2026-07-15: LLVM 18 syntax: no comma before ordering, align required
    writeln!(out, "{}store atomic i64 {}, ptr {} seq_cst, align 8", indent, val, ptr).ok();
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::void() }
}

fn emit_atomic_cas(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let addr = emit_arg(backend, out, &args[0], indent);
    let ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr).ok();
    let exp = emit_arg(backend, out, &args[1], indent);
    let des = emit_arg(backend, out, &args[2], indent);
    let cx = backend.fun.gen_reg();
    writeln!(out, "{}{} = cmpxchg ptr {}, i64 {}, i64 {} seq_cst seq_cst", indent, cx, ptr, exp, des).ok();
    // 2026-07-15: cmpxchg returns {i64, i1} — extract the value
    writeln!(out, "{}{} = extractvalue {{ i64, i1 }} {}, 0", indent, v, cx).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

fn emit_atomic_xchg(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let addr = emit_arg(backend, out, &args[0], indent);
    let ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr).ok();
    let val = emit_arg(backend, out, &args[1], indent);
    writeln!(out, "{}{} = atomicrmw xchg ptr {}, i64 {} seq_cst", indent, v, ptr, val).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

fn emit_atomic_add(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let addr = emit_arg(backend, out, &args[0], indent);
    let ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr).ok();
    let val = emit_arg(backend, out, &args[1], indent);
    writeln!(out, "{}{} = atomicrmw add ptr {}, i64 {} seq_cst", indent, v, ptr, val).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

fn emit_fence(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    writeln!(out, "{}fence seq_cst", indent).ok();
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
    BTypedRegister { name: v.to_string(), ty: Type::void() }
}

// ─── Dynamic linker intrinsics ───────────────────────────────────────
// 2026-07-15: dlopen/dlsym/dlclose are C library functions, not syscalls.
// The backend emits calls to @dlopen/@dlsym/@dlclose which are resolved
// by the system linker at load time.

fn emit_dl_open(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let path = emit_arg(backend, out, &args[0], indent);
    let flags = emit_arg(backend, out, &args[1], indent);
    writeln!(out, "{}{} = call ptr @dlopen(ptr {}, i32 {})", indent, v, path, flags).ok();
    BTypedRegister { name: v.to_string(), ty: Type::ptr(Type::bits(8)) }
}

fn emit_dl_sym(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let handle = emit_arg(backend, out, &args[0], indent);
    let symbol = emit_arg(backend, out, &args[1], indent);
    writeln!(out, "{}{} = call ptr @dlsym(ptr {}, ptr {})", indent, v, handle, symbol).ok();
    BTypedRegister { name: v.to_string(), ty: Type::ptr(Type::bits(8)) }
}

fn emit_dl_close(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let handle = emit_arg(backend, out, &args[0], indent);
    writeln!(out, "{}{} = call i32 @dlclose(ptr {})", indent, v, handle).ok();
    let ext = backend.fun.gen_reg();
    writeln!(out, "{}{} = sext i32 {} to i64", indent, ext, v).ok();
    BTypedRegister { name: ext.to_string(), ty: Type::int() }
}

// ─── Backtrace intrinsic ─────────────────────────────────────────────
// 2026-07-15: backtrace() walks the stack. Emits call to C runtime
// function @brief_backtrace() which uses glibc's backtrace().

fn emit_backtrace(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    writeln!(out, "{}{} = call i64 @brief_backtrace()", indent, v).ok();
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
