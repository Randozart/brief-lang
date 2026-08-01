// ── Intrinsic Call Expression Codegen ─────────────────────────────────
// 2026-07-14: Config-driven operation dispatch. Uses config/llvm-ops.toml
// for generic operations (Add#, Eq#, etc.) and special-case helpers for
// memory/I/O intrinsics (Malloc#, Print#, etc.) that don't fit templates.
// Flat code: max 2 nesting depth.

use std::sync::LazyLock;
use crate::ast::{Expr, Type};
use crate::backend::llvm::{AllocStrategy, LlvmBackend, TypedRegister as BTypedRegister};
use crate::config::AllocConfig;
use std::fmt::Write;

pub(crate) static ALLOC_CONFIG: LazyLock<AllocConfig> = LazyLock::new(|| AllocConfig::load());

/// Emit an intrinsic call by name. For generic operations (Add#, Eq#, etc.)
/// looks up the IR template from config/llvm-ops.toml using (op, primitive, bytes)
/// of the first argument. For memory/I/O intrinsics, uses special-case helpers.
pub fn emit_intrinsic_call(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    name: &str,
    args: &[Expr],
    analysis_id: Option<usize>,
    indent: &str,
) -> BTypedRegister {
    // Special-case intrinsics that don't fit the template pattern
    match name {
        "Malloc#" => return emit_malloc(backend, out, v, args, indent),
        "Alloc#" => return emit_alloc(backend, out, v, args, indent, analysis_id),
        "Free#" => return emit_free(backend, out, v, args, indent),
        "Load#" => return emit_load(backend, out, v, args, indent),
        "Store#" => return emit_store(backend, out, v, args, indent),
        "Copy#" => return emit_copy(backend, out, v, args, indent),
        "Fill#" => return emit_fill(backend, out, v, args, indent),

        "GetEnv#" => return emit_get_env(backend, out, v, args, indent),
        "GetEnvInt#" => return emit_get_env_int(backend, out, v, args, indent),
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
        // 2026-07-28: Print intrinsics — emit correct LLVM call types directly
        // (not through emit_external_call, which lacks type coercion for floats).
        "PrintInt#" => {
            let a = backend.emit_expr(out, &args[0], indent);
            let llvm_ty = backend.llvm_type(&a.ty);
            writeln!(out, "{}{} = call i64 @__print_int({} {})", indent, v, llvm_ty, a.name).ok();
            return BTypedRegister { name: v.to_string(), ty: Type::int() };
        }
        "PrintFloat#" => {
            let a = backend.emit_expr(out, &args[0], indent);
            // 2026-08-01 (C3): a boxed Float param (i64 handle boxed at defn
            // entry) must be unboxed through the float cache before the call —
            // llvm_type() reports "float" from the brief type, but the register
            // is really i64, and passing the handle to __print_float is an ABI
            // mismatch (`float %ac0` where %ac0 is i64).
            let unboxed = backend.fun.reg_float_cache.get(&a.name).cloned()
                .unwrap_or_else(|| a.name.clone());
            let (arg_llvm, fn_name) = if a.ty == Type::float64() {
                ("double", "__print_float64")
            } else {
                ("float", "__print_float")
            };
            writeln!(out, "{}{} = call i64 @{}({} {})", indent, v, fn_name, arg_llvm, unboxed).ok();
            return BTypedRegister { name: v.to_string(), ty: Type::int() };
        }
        "PrintChar#" => {
            let a = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}{} = call i64 @__print_char(i64 {})", indent, v, a.name).ok();
            return BTypedRegister { name: v.to_string(), ty: Type::int() };
        }
        "PrintStr#" => {
            let a = backend.emit_expr(out, &args[0], indent);
            // 2026-08-01 (B0): A Brief String value IS the ptr to a
            // length-prefixed [len][bytes] buffer, and __print_str takes
            // that pointer (the runtime's int64_t msg_bstr is the address,
            // typed as const char* in brief_rt.c to match this ABI). The
            // value register is already that ptr (literals emit it directly,
            // state loads inttoptr it via the state adapter, frgns return it).
            // Undo: if the ptr representation is ever reverted to an i64
            // handle, restore the ptrtoint boxing here and in
            // emit_legacy_string_literal.
            writeln!(out, "{}{} = call i64 @__print_str(ptr {})", indent, v, a.name).ok();
            return BTypedRegister { name: v.to_string(), ty: Type::int() };
        }
        // 2026-07-18: Pointer operations — special-case because they need
        // type-dependent codegen (Deref# needs pointee type, Index# needs
        // element type, Cast# needs target type). Ptr# is a simple inttoptr.
        "Deref#" => return emit_intrinsic_deref(backend, out, v, args, indent),
        "Index#" => return emit_intrinsic_index(backend, out, v, args, indent),
        "Cast#" => return emit_intrinsic_cast(backend, out, v, args, indent),
        "Ptr#" => {
            let a = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, v, a.name).ok();
            return BTypedRegister { name: v.to_string(), ty: Type::ptr(Type::int()) };
        }
        _ => {}
    }

    // For generic operations, look up template from config
    let arg_regs: Vec<BTypedRegister> = args.iter()
        .map(|a| backend.emit_expr(out, a, indent))
        .collect();

    if arg_regs.is_empty() {
        return BTypedRegister { name: v.to_string(), ty: Type::void() };
    }

    // Determine llvm type and bytes from the first argument's type
    // 2026-07-20: Hashword protocol — reads llvm_type from universe.
    let llvm_ty = backend.llvm_type(&arg_regs[0].ty);
    let bytes = resolve_arg_bytes(backend, &arg_regs[0]).unwrap_or(8);

    let op_name = name.trim_end_matches('#');
    // 2026-07-17: Directly emit float intrinsics (Sqrt#, Sin#, Cos#, etc.)
    let is_float_unary = matches!(op_name, "Sqrt" | "Sin" | "Cos" | "Fabs" | "Ceil" | "Floor");
    if is_float_unary {
        let llvm_name = op_name.to_lowercase();
        let (float_suffix, float_llvm_ty, ret_ty) = match llvm_ty.as_str() {
            "double" => ("f64", "double", Type::float64()),
            _ => ("f32", "float", Type::float()),
        };
        writeln!(out, "{}{} = call {} @llvm.{}.{}({} {})",
            indent, v, float_llvm_ty, llvm_name, float_suffix, float_llvm_ty, arg_regs[0].name).ok();
        return BTypedRegister { name: v.to_string(), ty: ret_ty };
    }

    // 2026-07-20: Simple hardcoded template dispatch for standard ops.
    // Replaces the old TOML config lookup. Phase 3 will replace this with
    // proper hashword category dispatch from op signatures.
    if let Some(template) = template_for_op(op_name, &llvm_ty, bytes) {
        let ir = template
            .replace("%v", v)
            .replace("%a", &arg_regs.get(0).map(|r| r.name.clone()).unwrap_or_default())
            .replace("%b", &arg_regs.get(1).map(|r| r.name.clone()).unwrap_or_default())
            .replace("%c", &arg_regs.get(2).map(|r| r.name.clone()).unwrap_or_default());
        writeln!(out, "{}  {}", indent, ir).ok();
        let ret_ty = if arg_regs.len() >= 1 {
            if arg_regs[0].ty == Type::float64() { Type::float64() }
            else if arg_regs[0].ty == Type::float() { Type::float() }
            else { Type::int() }
        } else { Type::int() };
        return BTypedRegister { name: v.to_string(), ty: ret_ty };
    }

    // Fallback: emit as external call
    emit_external_call(backend, out, v, name, args, indent)
}

/// 2026-07-20: Simple IR template dispatch, replacing the old TOML config lookup.
/// Produces the same IR templates that config/llvm-ops.toml provided, but
/// driven by the type's llvm_type rather than CTD metadata.
/// Phase 3 will replace this with proper hashword category dispatch.
pub(crate) fn template_for_op(op_name: &str, llvm_ty: &str, bytes: u64) -> Option<String> {
    let is_float = matches!(llvm_ty, "float" | "double" | "half" | "bfloat" | "fp128");
    let float_llvm = match llvm_ty {
        "float" | "half" | "bfloat" => "float",
        "double" => "double",
        _ if bytes <= 4 => "float",
        _ => "double",
    };
    // 2026-07-25: Use the passed llvm_ty for integer ops (may be narrowed
    // from value-range inference), falling back to bytes*8 if llvm_ty
    // doesn't look like an integer type (e.g., "ptr", "float").
    let int_llvm = if llvm_ty.starts_with('i') { llvm_ty.to_string() } else { format!("i{}", bytes * 8) };

    match (op_name, is_float) {
        ("Add", true) => Some(format!("%v = fadd fast {} %a, %b", float_llvm)),
        ("Sub", true) => Some(format!("%v = fsub fast {} %a, %b", float_llvm)),
        ("Mul", true) => Some(format!("%v = fmul fast {} %a, %b", float_llvm)),
        ("Div", true) => Some(format!("%v = fdiv fast {} %a, %b", float_llvm)),
        ("Rem", true) => Some(format!("%v = frem fast {} %a, %b", float_llvm)),
        ("Eq", true) => Some(format!("%v = fcmp oeq {} %a, %b", float_llvm)),
        ("Neq", true) => Some(format!("%v = fcmp une {} %a, %b", float_llvm)),
        ("Lt", true) => Some(format!("%v = fcmp olt {} %a, %b", float_llvm)),
        ("Gt", true) => Some(format!("%v = fcmp ogt {} %a, %b", float_llvm)),
        ("Le", true) => Some(format!("%v = fcmp ole {} %a, %b", float_llvm)),
        ("Ge", true) => Some(format!("%v = fcmp oge {} %a, %b", float_llvm)),
        ("Neg", true) => Some(format!("%v = fneg fast {} %a", float_llvm)),
        ("Abs", true) => Some(format!("%v = call {} @llvm.fabs.{}({} %a)", float_llvm, float_llvm, float_llvm)),

        ("Add", false) => Some(format!("%v = add nsw {} %a, %b", int_llvm)),
        ("Sub", false) => Some(format!("%v = sub nsw {} %a, %b", int_llvm)),
        ("Mul", false) => Some(format!("%v = mul nsw {} %a, %b", int_llvm)),
        ("Div", false) => Some(format!("%v = sdiv {} %a, %b", int_llvm)),
        ("Rem", false) => Some(format!("%v = srem {} %a, %b", int_llvm)),
        ("Eq", false) => Some(format!("%v = icmp eq {} %a, %b", int_llvm)),
        ("Neq", false) => Some(format!("%v = icmp ne {} %a, %b", int_llvm)),
        ("Lt", false) => Some(format!("%v = icmp slt {} %a, %b", int_llvm)),
        ("Gt", false) => Some(format!("%v = icmp sgt {} %a, %b", int_llvm)),
        ("Le", false) => Some(format!("%v = icmp sle {} %a, %b", int_llvm)),
        ("Ge", false) => Some(format!("%v = icmp sge {} %a, %b", int_llvm)),
        ("Neg", false) => Some(format!("%v = sub nsw {} 0, %a", int_llvm)),
        ("Abs", false) => Some(format!("%v = call {} @llvm.abs.{}({} %a, i1 false)", int_llvm, int_llvm, int_llvm)),

        ("BitAnd", false) => Some(format!("%v = and {} %a, %b", int_llvm)),
        ("BitOr", false) => Some(format!("%v = or {} %a, %b", int_llvm)),
        ("BitXor", false) => Some(format!("%v = xor {} %a, %b", int_llvm)),
        ("Shl", false) => Some(format!("%v = shl {} %a, %b", int_llvm)),
        ("Shr", false) => Some(format!("%v = ashr {} %a, %b", int_llvm)),

        _ => None,
    }
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
    // 2026-07-18: Record Malloc strategy so Free# can dispatch correctly.
    backend.fun.alloc_strategies.insert(v.to_string(), AllocStrategy::Malloc);
    // 2026-07-18: Record fat pointer provenance — base points to alloc,
    // offset 0, remaining = size. This enables O(1) Length#(ptr).
    let remaining_reg = backend.fun.gen_reg();
    writeln!(out, "{} {} = add i64 {}, 0", indent, remaining_reg, size).ok();
    backend.fun.fat_ptrs.insert(v.to_string(), (v.to_string(), "0".to_string(), remaining_reg));
    BTypedRegister { name: v.to_string(), ty: Type::ptr(Type::int()) }
}

// 2026-07-18: Alloc# — compiler-delegated allocation with triple dispatch.
// Args:
//   Alloc#(size)                        — compiler picks (scope-based)
//   Alloc#(size, Arena)                 — PascalCase: intrinsic dispatch
//   Alloc#(size, Malloc)                — PascalCase: intrinsic dispatch
//   Alloc#(size, Alloca)                — PascalCase: intrinsic dispatch
//   Alloc#(size, "pool_serial")         — quoted: config/alloc-strategies.toml
//   Alloc#(size, my_custom_alloc_fn)    — identifier: user Brief function
fn emit_alloc(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str, analysis_id: Option<usize>,
) -> BTypedRegister {
    let size = emit_arg(backend, out, &args[0], indent);

    // 2026-07-18: Phase 4 — Check pre-computed strategy from analysis pass.
    // If the analysis determined Malloc (escape detected), use it directly.
    if let Some(aid) = analysis_id {
        if let Some(ref strategies) = backend.analysis_alloc_strategies {
            if let Some(strategy) = strategies.get(&aid) {
                return match strategy {
                    AllocStrategy::Malloc => {
                        // TEMP: 2026-07-18: Conservative — always Malloc for now.
                        // Full escape analysis will assign Arena/Alloca when safe.
                        emit_malloc_inline(backend, out, v, &size, indent)
                    }
                    AllocStrategy::Arena => {
                        let result = backend.emit_arena_alloc(out, indent, &size);
                        writeln!(out, "{}{} = add i64 0, {}", indent, v, result).ok();
                        backend.fun.alloc_strategies.insert(v.to_string(), AllocStrategy::Arena);
                        BTypedRegister { name: v.to_string(), ty: Type::int() }
                    }
                    AllocStrategy::Alloca => {
                        let a = format!("%alloc_{}", backend.fun.txn_counter);
                        backend.fun.txn_counter += 1;
                        writeln!(out, "{}{} = alloca i8, i64 {}", indent, a, size).ok();
                        writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, a).ok();
                        backend.fun.alloc_strategies.insert(v.to_string(), AllocStrategy::Alloca);
                        BTypedRegister { name: v.to_string(), ty: Type::int() }
                    }
                    // 2026-07-18: Inline — allocation fits in parent struct field.
                    // The Alloc# is a no-op; the address is computed from the
                    // containing struct's field offset at access time.
                    AllocStrategy::Inline => {
                        backend.fun.alloc_strategies.insert(v.to_string(), AllocStrategy::Inline);
                        writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                        BTypedRegister { name: v.to_string(), ty: Type::int() }
                    }
                    // 2026-07-18: RingBuffer — circular buffer with wrap-around.
                    AllocStrategy::RingBuffer => {
                        return emit_ring_buffer_alloc(backend, out, v, &size, indent);
                    }
                    // 2026-07-18: Config strategy — look up template.
                    AllocStrategy::Config(_) | AllocStrategy::Custom(_) => {
                        backend.fun.alloc_strategies.insert(v.to_string(), strategy.clone());
                        emit_malloc_inline(backend, out, v, &size, indent)
                    }
                };
            }
        }
    }

    // Check for optional 2nd arg (strategy override) — explicit user override
    // takes priority over analysis.
    if args.len() >= 2 {
        return emit_alloc_with_strategy(backend, out, v, args, indent, &size);
    }
    // Default triple dispatch (no strategy arg).
    // Strategy 1: Arena scope active → bump allocate.
    // 2026-07-19: Arena is in %State fields — available in any function that
    // has %state (all txns, callable txns, and their helpers by inheritance).
    if backend.arena_ptr_idx.is_some() {
        // 2026-07-19: emit_arena_alloc returns the old bump pointer as i64.
        // The caller receives it directly — no ptrtoint needed.
        let result = backend.emit_arena_alloc(out, indent, &size);
        writeln!(out, "{}{} = add i64 0, {}", indent, v, result).ok();
        backend.fun.alloc_strategies.insert(v.to_string(), AllocStrategy::Arena);
        return BTypedRegister { name: v.to_string(), ty: Type::int() };
    }
    if backend.is_in_bounded_scope() && !backend.will_escape_current_allocation() {
        // 2026-07-18: Check if size is a compile-time constant.
        let is_constant = matches!(&args[0], Expr::Decimal(_));
        if is_constant {
            let a = format!("%alloc_{}", backend.fun.txn_counter);
            backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = alloca i8, i64 {}", indent, a, size).ok();
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, a).ok();
            backend.fun.alloc_strategies.insert(v.to_string(), AllocStrategy::Alloca);
            return BTypedRegister { name: v.to_string(), ty: Type::int() };
        }
        // Runtime fallback: try alloca, fall back to malloc if size > threshold.
        return emit_dynamic_alloc(backend, out, v, &size, indent);
    }
    // Strategy 3: Default → @malloc.
    emit_malloc_inline(backend, out, v, &size, indent)
}

// 2026-07-18: Handle explicit strategy override for Alloc#.
// Strategy can be PascalCase (Arena/Malloc/Alloca), quoted string (config),
// or an identifier (user function).
fn emit_alloc_with_strategy(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str, size: &str,
) -> BTypedRegister {
    let strategy_expr = &args[1];
    match strategy_expr {
        Expr::Identifier(name) => {
            match name.as_str() {
                "Arena" => {
                    let result = backend.emit_arena_alloc(out, indent, size);
                    // 2026-07-19: emit_arena_alloc returns i64 — route to v.
                    writeln!(out, "{}{} = add i64 0, {}", indent, v, result).ok();
                    backend.fun.alloc_strategies.insert(v.to_string(), AllocStrategy::Arena);
                }
                "Malloc" => {
                    emit_malloc_inline(backend, out, v, size, indent);
                }
                "Alloca" => {
                    writeln!(out, "{}{} = alloca i8, i64 {}", indent, v, size).ok();
                    backend.fun.alloc_strategies.insert(v.to_string(), AllocStrategy::Alloca);
                }
                // Unknown PascalCase — treat as user function name.
                custom_fn => {
                    let fn_reg = emit_arg(backend, out, strategy_expr, indent);
                    writeln!(out, "{}{} = call i64 @{}(i64 {})", indent, v, custom_fn, size).ok();
                    // Conservative: unknown strategy → Malloc for Free# dispatch.
                    backend.fun.alloc_strategies.insert(v.to_string(), AllocStrategy::Malloc);
                }
            }
        }
        Expr::Quoted(bytes) => {
            let strategy_name = String::from_utf8_lossy(bytes).to_string();
            // Look up in config/alloc-strategies.toml.
            let found = emit_alloc_from_config(backend, out, v, &strategy_name, size, indent);
            if !found {
                // Fallback to @malloc with warning.
                let msg = format!("warning: unknown alloc strategy '{}', falling back to malloc", strategy_name);
                backend.warnings.push(msg);
                emit_malloc_inline(backend, out, v, size, indent);
            }
        }
        _ => {
            // Unknown expression type — emit as function call.
            let fn_reg = emit_arg(backend, out, strategy_expr, indent);
            writeln!(out, "{}{} = call i64 @custom_alloc(i64 {}, i64 {})", indent, v, size, fn_reg).ok();
            backend.fun.alloc_strategies.insert(v.to_string(), AllocStrategy::Malloc);
        }
    }
    BTypedRegister { name: v.to_string(), ty: Type::ptr(Type::int()) }
}

// 2026-07-18: Look up a quoted strategy name in config/alloc-strategies.toml
// and emit the corresponding LLVM IR template. Returns true if found.
fn emit_alloc_from_config(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    strategy_name: &str, size: &str, indent: &str,
) -> bool {
    let config = crate::config::AllocConfig::load();
    let Some(template) = config.lookup(strategy_name) else {
        return false;
    };
    let ir = template
        .replace("{v}", v.trim_start_matches('%'))
        .replace("{size}", size);
    writeln!(out, "{}  {}", indent, ir).ok();
    // AllocConfig entries default to Malloc strategy for Free# dispatch.
    backend.fun.alloc_strategies.insert(v.to_string(), AllocStrategy::Malloc);
    true
}

// 2026-07-18: Runtime fallback — try stack (alloca), fall back to heap
// if the allocation size exceeds the stack threshold. Used for dynamic-size
// allocs where the strategy is Alloca but size is unknown at compile time.
fn emit_dynamic_alloc(
    backend: &mut LlvmBackend, out: &mut String, v: &str, size: &str, indent: &str,
) -> BTypedRegister {
    let counter = backend.fun.txn_counter;
    backend.fun.txn_counter += 1;
    let stack_l = format!(".stack{}", counter);
    let heap_l = format!(".heap{}", counter);
    let done_l = format!(".done{}", counter);

    writeln!(out, "  %cmp = icmp ule i64 {}, {}", size, backend.ctx.stack_threshold).ok();
    writeln!(out, "  br i1 %cmp, label %{}, label %{}", stack_l, heap_l).ok();
    writeln!(out, "{}:", stack_l).ok();
    writeln!(out, "  %s = alloca i8, i64 {}", size).ok();
    writeln!(out, "  %sv = ptrtoint ptr %s to i64").ok();
    writeln!(out, "  br label %{}", done_l).ok();
    writeln!(out, "{}:", heap_l).ok();
    writeln!(out, "  %h = call ptr @malloc(i64 {})", size).ok();
    writeln!(out, "  %hv = ptrtoint ptr %h to i64").ok();
    writeln!(out, "  br label %{}", done_l).ok();
    writeln!(out, "{}:", done_l).ok();
    writeln!(out, "  {} = phi i64 [ %sv, %{} ], [ %hv, %{} ]", v, stack_l, heap_l).ok();
    backend.fun.alloc_strategies.insert(v.to_string(), AllocStrategy::Alloca);
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// 2026-07-18: RingBuffer — circular buffer alloc via slot-based ring.
// Each allocation advances a head pointer modulo RING_SIZE (power of two).
// Free# is a no-op — old slots are overwritten when head wraps around.
fn emit_ring_buffer_alloc(
    backend: &mut LlvmBackend, out: &mut String, v: &str, size: &str, indent: &str,
) -> BTypedRegister {
    let counter = backend.fun.txn_counter;
    backend.fun.txn_counter += 1;
    // Ring buffer uses a stack-allocated circular buffer per txn scope.
    // For now: emit as alloca and mark as RingBuffer for Free# behavior.
    // Full implementation would use @ring_head global + wrapping GEP.
    let a = format!("%ring_buf{}", counter);
    writeln!(out, "{}{} = alloca i8, i64 {}", indent, a, size).ok();
    writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, a).ok();
    backend.fun.alloc_strategies.insert(v.to_string(), AllocStrategy::RingBuffer);
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// 2026-07-18: Emit @malloc for a size register, returning ptrtoint'd i64.
fn emit_malloc_inline(
    backend: &mut LlvmBackend, out: &mut String, v: &str, size: &str, indent: &str,
) -> BTypedRegister {
    let name = v.trim_start_matches('%');
    writeln!(out, "{}%{}_p = call ptr @malloc(i64 {})", indent, name, size).ok();
    writeln!(out, "{}{} = ptrtoint ptr %{}_p to i64", indent, v, name).ok();
    backend.fun.alloc_strategies.insert(v.to_string(), AllocStrategy::Malloc);
    let remaining_reg = backend.fun.gen_reg();
    writeln!(out, "{} {} = add i64 {}, 0", indent, remaining_reg, size).ok();
    backend.fun.fat_ptrs.insert(v.to_string(), (v.to_string(), "0".to_string(), remaining_reg));
    // 2026-07-18: Alloc# returns i64 (ptrtroint), not ptr.
    // The register already holds ptrtoint ptr %malloc_p to i64.
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}

// 2026-07-18: Free# — strategy-aware. Looks up the pointer's allocation
// strategy from the alloc_strategies map. Arena/Alloca → no-op (memory
// reclaimed by scope end / arena reset). Malloc/unknown → call @free.
fn emit_free(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let ptr_reg = emit_arg(backend, out, &args[0], indent);
    // Look up the allocation strategy for the pointer register.
    let strategy = backend.fun.alloc_strategies.get(&ptr_reg);
    match strategy {
        // 2026-07-18: Inline, RingBuffer, Arena, Alloca — no Free# needed.
        Some(AllocStrategy::Arena) | Some(AllocStrategy::Alloca)
            | Some(AllocStrategy::Inline) | Some(AllocStrategy::RingBuffer) => {}
        // 2026-07-18: Config strategy — check the free field from config.
        Some(AllocStrategy::Config(name)) => {
            match ALLOC_CONFIG.lookup_free(name) {
                Some("none") => {}  // no-op
                Some(fn_name) => {   // custom free function
                    writeln!(out, "{}call void @{}(ptr {})", indent, fn_name, ptr_reg).ok();
                }
                None => {            // default → @free
                    writeln!(out, "{}call void @free(ptr {})", indent, ptr_reg).ok();
                }
            }
        }
        Some(AllocStrategy::Malloc) | Some(AllocStrategy::Custom(_)) | _ => {
            // Heap-allocated (Malloc) or unknown → emit @free.
            // 2026-08-01 (D2): a Ptr value is stored as an i64 handle (ptrtoint
            // at store); the handle must be inttoptr'd before the @free call.
            let p = backend.fun.gen_reg();
            writeln!(out, "{}  {} = inttoptr i64 {} to ptr", indent, p, ptr_reg).ok();
            writeln!(out, "{}call void @free(ptr {})", indent, p).ok();
        }
    }
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
    // 2026-07-18: Narrow loads (< 8 bytes) are zero-extended to i64 so the
    // result matches the declared return type (Int = i64). Without this,
    // comparisons of loaded bytes fail (icmp expects i64, got i8).
    if bytes < 8 {
        let zext = backend.fun.gen_reg();
        writeln!(out, "{}{} = zext i{} {} to i64", indent, zext, bytes * 8, v).ok();
        return BTypedRegister { name: zext, ty: Type::int() };
    }
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

// 2026-07-19: GetEnv# returns the raw env var value as a String.
// Returns empty string {0, 0} if the env var is not found.
fn emit_get_env(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let name_reg = emit_arg(backend, out, &args[0], indent);
    let ptr_reg = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr_reg, name_reg).ok();
    // Call getenv — may return null
    let env_ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = call ptr @getenv(ptr {})", indent, env_ptr, ptr_reg).ok();
    let is_null = backend.fun.gen_reg();
    writeln!(out, "{}{} = icmp eq ptr {}, null", indent, is_null, env_ptr).ok();
    // Allocate a fallback 1-byte null-terminated buffer for the null case
    let fb = backend.fun.gen_reg();
    writeln!(out, "{}{} = alloca i8, i64 1", indent, fb).ok();
    writeln!(out, "{}store i8 0, ptr {}", indent, fb).ok();
    // Safe pointer: fallback buffer when null, real pointer otherwise
    let safe_ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = select i1 {}, ptr {}, ptr {}", indent, safe_ptr, is_null, fb, env_ptr).ok();
    // Compute length via strlen on the safe pointer
    let len = backend.fun.gen_reg();
    writeln!(out, "{}{} = call i64 @strlen(ptr {})", indent, len, safe_ptr).ok();
    // Data: 0 when null, ptrtoint(env_ptr) otherwise
    let data_raw = backend.fun.gen_reg();
    writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, data_raw, env_ptr).ok();
    let data = backend.fun.gen_reg();
    writeln!(out, "{}{} = select i1 {}, i64 0, i64 {}", indent, data, is_null, data_raw).ok();
    // Pack into {i64, i64} SSO string struct
    let t1 = backend.fun.gen_reg();
    writeln!(out, "{}{} = insertvalue {{ i64, i64 }} undef, i64 {}, 0", indent, t1, data).ok();
    let t2 = backend.fun.gen_reg();
    writeln!(out, "{}{} = insertvalue {{ i64, i64 }} %{}, i64 {}, 1", indent, t2, t1, len).ok();
    BTypedRegister { name: t2, ty: Type::string() }
}

// 2026-07-19: GetEnvInt# returns the env var value parsed as Int.
// Returns 0 if the env var is missing or unparseable (matches atol behavior).
fn emit_get_env_int(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let name_reg = emit_arg(backend, out, &args[0], indent);
    let ptr_reg = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr_reg, name_reg).ok();
    // 2026-07-28: Brief strings are stored as [i64 length][data\0] with the
    // handle pointing to the struct start. getenv expects just the data portion.
    // Without this GEP, getenv reads the length field as the string (e.g.,
    // length=5 → binary 0x05 → empty string) → returns NULL → atol(NULL)
    // segfaults. This was the root cause of the popcount binary crash.
    let data_ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 8", indent, data_ptr, ptr_reg).ok();
    let env_ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = call ptr @getenv(ptr {})", indent, env_ptr, data_ptr).ok();
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
    let arg_reg = emit_arg(backend, out, &args[0], indent);
    // Check if this argument has fat pointer provenance — if so, read
    // the remaining length directly from the provenance metadata.
    if let Some((_base, _offset, ref remaining)) = backend.fun.fat_ptrs.get(&arg_reg).cloned() {
        writeln!(out, "{}{} = add i64 {}, 0", indent, v, remaining).ok();
        return BTypedRegister { name: v.to_string(), ty: Type::int() };
    }
    // Fallback: load length from string/list header slot 0.
    let ptr = backend.fun.gen_reg();
    writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, arg_reg).ok();
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
        "ChMod" => 90, "ChOwn" => 92, "Clone" => 56,
        "GetEgid" => 108, "GetEuid" => 107, "GetGid" => 104,
        "GetPgid" => 109, "GetSid" => 124,
        "GetSockOpt" => 55, "GetUid" => 102, "Mlock" => 149,
        "Mprotect" => 10, "SetSockOpt" => 54,
        "ShmGet" => 29, "Shutdown" => 48, "UMask" => 95,
        "ShmAt" => 30, "ShmDt" => 31, "SemGet" => 64,
        "SemOp" => 65, "SemCtl" => 66, "ClockGetTime" => 228,
        "ClockSetTime" => 229, "Futex" => 202,
        "GetRandom" => 318, "Openat" => 257,
        "Membarrier" => 324, "CopyFileRange" => 326,
        "PRead" => 17, "PWrite" => 18,
        _ => return None,
    })
}

/// 2026-07-26: Emit SysCall# — first arg is op (Int raw number or PascalCase
/// abstract name), followed by up to 6 Int arguments.
/// On x86_64/aarch64 Linux: emits inline assembly (syscall/svc #0).
/// On other targets: falls back to @brief_syscall from brief_rt.c.
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
        Expr::Decimal(n) => format!("{}", n),
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
    let triple = &backend.ctx.target_triple;
    if triple.starts_with("x86_64") && triple.contains("linux") {
        // x86_64 Linux: inline syscall instruction
        // syscall clobbers rcx (saves RIP) and r11 (saves RFLAGS).
        // Args in rax, rdi, rsi, rdx, r10, r8, r9. Output in rax.
        writeln!(out, "{}{} = call i64 asm sideeffect \"syscall\", \"={{rax}},{{rax}},{{rdi}},{{rsi}},{{rdx}},{{r10}},{{r8}},{{r9}},~{{rcx}},~{{r11}}\" ({}",
            indent, v, all_args.join(", ")).ok();
        writeln!(out, "{}  )", indent).ok();
    } else if triple.starts_with("aarch64") && triple.contains("linux") {
        // aarch64 Linux: inline svc #0
        // Args in x0-x5, output in x0. No clobbers beyond the ABI (kernel preserves).
        writeln!(out, "{}{} = call i64 asm sideeffect \"svc #0\", \"={{x0}},{{x0}},{{x1}},{{x2}},{{x3}},{{x4}},{{x5}}\" ({}",
            indent, v, all_args.join(", ")).ok();
        writeln!(out, "{}  )", indent).ok();
    } else {
        // Non-Linux fallback: call brief_syscall via C runtime
        writeln!(out, "{}{} = call i64 @brief_syscall({})", indent, v, all_args.join(", ")).ok();
    }
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

// 2026-07-18: Deref# — load through pointer. The pointee type is resolved
// from the ptr argument's Type::Ptr(inner) and used as the LLVM load type.
fn emit_intrinsic_deref(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let ptr_reg = backend.emit_expr(out, &args[0], indent);
    let inner_ty = match &ptr_reg.ty { Type::Ptr(i) => *i.clone(), _ => Type::int() };
    let llvm_ty = backend.llvm_type(&inner_ty);
    writeln!(out, "{}{} = load {}, ptr {}, align {}", indent, v, llvm_ty, ptr_reg.name,
        backend.align_of(&llvm_ty)).ok();
    BTypedRegister { name: v.to_string(), ty: inner_ty }
}

// 2026-07-18: Index# — get element at index. GEP + load through pointer.
fn emit_intrinsic_index(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let obj_reg = backend.emit_expr(out, &args[0], indent);
    let idx_reg = backend.emit_expr(out, &args[1], indent);
    let inner_ty = match &obj_reg.ty { Type::Ptr(i) => *i.clone(), _ => Type::int() };
    let llvm_ty = backend.llvm_type(&inner_ty);
    let gep = backend.fun.gen_reg();
    writeln!(out, "{}{} = getelementptr {}, ptr {}, i64 {}", indent, gep, llvm_ty, obj_reg.name, idx_reg.name).ok();
    writeln!(out, "{}{} = load {}, ptr {}, align {}", indent, v, llvm_ty, gep,
        backend.align_of(&llvm_ty)).ok();
    BTypedRegister { name: v.to_string(), ty: inner_ty }
}

// ── Cast Resolution Pipeline ───────────────────────────────────────────
// 2026-07-30: Cast#(source, target) resolved by casting graph.
// Falls back to LLVM bitcast when no graph path exists.

fn emit_intrinsic_cast(
    backend: &mut LlvmBackend, out: &mut String, v: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    if args.len() < 2 { return BTypedRegister { name: v.to_string(), ty: Type::int() }; }
    let src = backend.emit_expr(out, &args[0], indent);

    // Extract target type from second argument
    let target = extract_type_from_expr(&args[1]).unwrap_or(Type::int());

    // Try casting graph path first
    if let Some(result) = backend.emit_cast_path(out, v, &src, &target, indent) {
        return BTypedRegister { name: result.name, ty: target };
    }

    // Fallback: LLVM bitcast
    let src_ll = backend.llvm_type(&src.ty);
    let target_ll = backend.llvm_type(&target);
    writeln!(out, "{}{} = bitcast {} {} to {}", indent, v, src_ll, src.name, target_ll).ok();
    BTypedRegister { name: v.to_string(), ty: target }
}

/// Extract a Type from an Expr that represents a type name.
fn extract_type_from_expr(expr: &Expr) -> Option<Type> {
    match expr {
        Expr::Identifier(name) => Some(Type::Custom(name.clone())),
        _ => None,
    }
}


fn emit_external_call(
    backend: &mut LlvmBackend, out: &mut String, v: &str, name: &str,
    args: &[Expr], indent: &str,
) -> BTypedRegister {
    let typed_regs: Vec<BTypedRegister> = args.iter()
        .map(|a| backend.emit_expr(out, a, indent))
        .collect();
    // 2026-07-25: External calls expect i64 arguments. Always pass as i64
    // to match the C ABI. The type checker may narrow to i8/i16/i32, but
    // the actual SSA value is already i64 from ptrtoint.
    let arg_strs: Vec<String> = typed_regs.iter().map(|reg| {
        format!("i64 {}", reg.name)
    }).collect();
    let clean_name = name.trim_end_matches('#');
    writeln!(out, "{}{} = call i64 @{}({})", indent, v, clean_name, arg_strs.join(", ")).ok();
    BTypedRegister { name: v.to_string(), ty: Type::int() }
}
