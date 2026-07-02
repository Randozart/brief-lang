// ── Intrinsic Call Expression Codegen ─────────────────────────────
//
// Handles emission of IntrinsicCall expressions (math, string, I/O,
// system, threading, networking, etc.)
//
// 2026-06-29: Extracted from emit_expr.rs lines 666-2707 (~2000 lines).

use crate::ast::{Expr, Intrinsic, Type};
use crate::backend::llvm::{float_to_llvm_hex, float64_to_llvm_hex, LlvmBackend, TypedRegister};
use std::fmt::Write;

/// Emit an intrinsic call with the given arguments.
/// Handles ALL intrinsic variants.
pub fn emit_intrinsic_call(
    backend: &mut LlvmBackend,
    out: &mut String,
    v: &str,
    intrinsic: &Intrinsic,
    args: &[Expr],
    indent: &str,
) -> TypedRegister {
    let emit_intrinsic_float_unary = |backend: &mut LlvmBackend, out: &mut String, indent: &str, v: &str, llvm_name: &str, arg: &Expr| -> TypedRegister {
        let raw = backend.emit_expr(out, arg, indent);
        let fl = backend.ensure_float_reg(out, indent, &raw);
        // 2026-06-29: Dispatch to f64 variant for Float64 args, f32 for Float args
        if raw.ty == Type::Float64 {
            writeln!(out, "{}{} = call double @llvm.{}.f64(double {})", indent, v, llvm_name, fl).ok();
            TypedRegister { name: v.to_string(), ty: Type::Float64 }
        } else {
            writeln!(out, "{}{} = call float @llvm.{}.f32(float {})", indent, v, llvm_name, fl).ok();
            TypedRegister { name: v.to_string(), ty: Type::Float }
        }
    };
    match intrinsic {
        Intrinsic::Sqrt => { return emit_intrinsic_float_unary(backend, out, indent, &v, "sqrt", &args[0]); }
        Intrinsic::Sin => { return emit_intrinsic_float_unary(backend, out, indent, &v, "sin", &args[0]); }
        Intrinsic::Cos => { return emit_intrinsic_float_unary(backend, out, indent, &v, "cos", &args[0]); }
        Intrinsic::Pow => {
            let a = backend.emit_expr(out, &args[0], indent);
            let b = backend.emit_expr(out, &args[1], indent);
            if a.ty == Type::Float64 {
                writeln!(out, "{}{} = call double @pow(double {}, double {})", indent, v, a.name, b.name).ok();
                return TypedRegister { name: v.to_string(), ty: Type::Float64 };
            }
            writeln!(out, "{}{} = call double @pow(double {}, double {})", indent, v, a.name, b.name).ok();
            return TypedRegister { name: v.to_string(), ty: Type::Float };
        }
        Intrinsic::Fabs => { return emit_intrinsic_float_unary(backend, out, indent, &v, "fabs", &args[0]); }
        Intrinsic::Ceil => { return emit_intrinsic_float_unary(backend, out, indent, &v, "ceil", &args[0]); }
        Intrinsic::Floor => { return emit_intrinsic_float_unary(backend, out, indent, &v, "floor", &args[0]); }
        Intrinsic::FloatToStr => {
            if !args.is_empty() {
                let a_raw = backend.emit_expr(out, &args[0], indent);
                let a_f = backend.ensure_float_reg(out, indent, &a_raw);
                // 2026-06-29: Float64 → __float64_to_str, Float → __float_to_str
                if a_raw.ty == Type::Float64 {
                    writeln!(out, "{}{} = call i64 @__float64_to_str(double {})", indent, v, a_f).ok();
                } else {
                    writeln!(out, "{}{} = call i64 @__float_to_str(float {})", indent, v, a_f).ok();
                }
            }
            return TypedRegister { name: v.to_string(), ty: Type::String };
        }
        Intrinsic::ToStr => {
            if !args.is_empty() {
                let a_raw = backend.emit_expr(out, &args[0], indent);
                writeln!(out, "{}{} = call i64 @__to_str(i64 {})", indent, v, a_raw.name).ok();
            }
            return TypedRegister { name: v.to_string(), ty: Type::String };
        }
        Intrinsic::Ctpop => {
            let raw = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}{} = call i64 @llvm.ctpop.i64(i64 {})", indent, v, raw).ok();
        }
        Intrinsic::Ctlz => {
            let raw = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}{} = call i64 @llvm.ctlz.i64(i64 {}, i1 false)", indent, v, raw).ok();
        }
        Intrinsic::Cttz => {
            let raw = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}{} = call i64 @llvm.cttz.i64(i64 {}, i1 false)", indent, v, raw).ok();
        }
        Intrinsic::Abs => {
            let raw = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}{} = call i64 @llvm.abs.i64(i64 {}, i1 false)", indent, v, raw).ok();
        }
        Intrinsic::Bitreverse => {
            let raw = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}{} = call i64 @llvm.bitreverse.i64(i64 {})", indent, v, raw).ok();
        }
        Intrinsic::ByteCount => {
            writeln!(out, "{}{} = add i64 0, 8 ; bytes", indent, v).ok();
        }
        Intrinsic::StrBytes => {
            if let Some(first) = args.first() {
                let n = backend.emit_expr(out, first, indent);
                let boxed = backend.adapt_to_i64(out, indent, &n);
                writeln!(out, "{}{} = call i64 @__str_bytes__(i64 {})", indent, v, boxed).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }
        Intrinsic::Size => {
            if let Some(first) = args.first() {
                let list_val = backend.emit_expr(out, first, indent);
                let list_boxed = backend.adapt_to_i64(out, indent, &list_val);
                let hp = format!("%ishp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_boxed).ok();
                let lp = format!("%islp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, v, lp).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }
        Intrinsic::Pop => {
            if let Some(first) = args.first() {
                let list_val = backend.emit_expr(out, first, indent);
                let list_boxed = backend.adapt_to_i64(out, indent, &list_val);
                let hp = format!("%ipphp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_boxed).ok();
                let lp = format!("%ipplp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                let len = format!("%ippln{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, len, lp).ok();
                let dpp = format!("%ippdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dpp, hp).ok();
                let pi = format!("%ippi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, -1", indent, pi, len).ok();
                let ep = format!("%ippep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, dpp, pi).ok();
                writeln!(out, "{}{} = load i64, i64* {}, align 8, !tbaa !1", indent, v, ep).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }
        Intrinsic::Contains => {
            if args.len() >= 2 {
                let list_val = backend.emit_expr(out, &args[0], indent);
                let elem_val = backend.emit_expr(out, &args[1], indent);
                let list_boxed = backend.adapt_to_i64(out, indent, &list_val);
                let elem_boxed = backend.adapt_to_i64(out, indent, &elem_val);
                let cmp = format!("%isc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, cmp, list_boxed, elem_boxed).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }
        Intrinsic::Keys | Intrinsic::Values => {
            if let Some(first) = args.first() {
                let list_val = backend.emit_expr(out, first, indent);
                let list_boxed = backend.adapt_to_i64(out, indent, &list_val);
                // Return the list as-is (Keys/Values of a List is the list itself)
                writeln!(out, "{}{} = add i64 0, {}", indent, v, list_boxed).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }
        // System I/O intrinsics (stubs — passthrough to frgn calls)
        Intrinsic::Println => {
            // Print a Brief String followed by newline.
            // Brief String value is i64 (ptrtoint of struct ptr).
            // Load the first field (ptr_to_data) to get the data pointer.
            // 2026-06-29: Strip tag bits (bit 0=static, bit 1=temporary)
            // before loading data_ptr. Without this, concat results tagged
            // with OR 2 read from offset +2 instead of offset 0, producing
            // a garbage data_ptr that crashes in fprintf's strlen.
            if !args.is_empty() {
                let msg = backend.emit_expr(out, &args[0], indent);
                let clean = format!("%pplc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = and i64 {}, -4", indent, clean, msg).ok();
                let sptr = format!("%ppls{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sptr, clean).ok();
                let sp = format!("%pplp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = bitcast ptr {} to i64*", indent, sp, sptr).ok();
                let data_ptr = format!("%ppld{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, data_ptr, sp).ok();
                let str_ptr = format!("%pplp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, str_ptr, data_ptr).ok();
                let so = format!("%pplo{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so).ok();
                let fmt = format!("%pplf{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr [4 x i8], [4 x i8]* @FMT_STR, i64 0, i64 0", indent, fmt).ok();
                let fr = format!("%ppfr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = call i32 (ptr, ptr, ...) @fprintf(ptr {}, ptr {}, ptr {})",
                    indent, fr, so, fmt, str_ptr).ok();
                let so2 = format!("%pplo{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so2).ok();
                writeln!(out, "{}{} = call i32 @fflush(ptr {})", indent, v, so2).ok();
            } else {
                writeln!(out, "{}{} = add i64 0, 1 ; println no arg", indent, v).ok();
            }
        }
        Intrinsic::Print => {
            // Print a Brief String WITHOUT newline.
            // Load hdr[0] (data pointer) and call fprintf.
            // 2026-06-29: Strip tag bits before loading data_ptr (same
            // rationale as Println — see comment above).
            if !args.is_empty() {
                let msg = backend.emit_expr(out, &args[0], indent);
                let clean = format!("%pplc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = and i64 {}, -4", indent, clean, msg).ok();
                let sptr = format!("%ppls{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sptr, clean).ok();
                let sp = format!("%pplp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = bitcast ptr {} to i64*", indent, sp, sptr).ok();
                let data_ptr = format!("%ppld{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, data_ptr, sp).ok();
                let str_ptr = format!("%pplp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, str_ptr, data_ptr).ok();
                let so = format!("%pplo{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so).ok();
                let fr = format!("%ppfr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = call i32 @fputs(ptr {}, ptr {})",
                    indent, v, str_ptr, so).ok();
            } else {
                writeln!(out, "{}{} = add i64 0, 1 ; print no arg", indent, v).ok();
            }
        }
        Intrinsic::Readln => {
            writeln!(out, "{}{} = call i64 @__readln__()", indent, v).ok();
        }
        Intrinsic::Exit => {
            // Call libc exit to terminate the program
            if args.len() >= 1 {
                let code = backend.emit_expr(out, &args[0], indent);
                let ct = format!("%pext{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = trunc i64 {} to i32", indent, ct, code).ok();
                writeln!(out, "{}call void @exit(i32 {})", indent, ct).ok();
            } else {
                writeln!(out, "{}call void @exit(i32 0)", indent).ok();
            }
            writeln!(out, "{}{} = add i64 0, 1", indent, v).ok();
        }
        Intrinsic::Time => {
            writeln!(out, "{}{} = call i64 @time(i64* null)", indent, v).ok();
        }
        Intrinsic::ReadFile => {
            // 2026-06-18: brief_read_file now returns i64 (Brief string ptr)
            // or 0 on failure. The C function handles the fopen/read/boxing.
            if args.len() >= 1 {
                let path_val = backend.emit_expr(out, &args[0], indent);
                let boxed = backend.adapt_to_i64(out, indent, &path_val);
                let raw = format!("%frraw{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = call i64 @__read_file__(i64 {})", indent, raw, boxed).ok();
                let is_zero = format!("%frisz{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = icmp eq i64 {}, 0", indent, is_zero, raw).ok();
                let el = format!("rf_err{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let ol = format!("rf_ok{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let dl = format!("rf_done{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, is_zero, el, ol).ok();

                // Err("file not found") — packed Result: disc=1 low 8 bits,
                // payload=ptrtoint(@STR_READFILE_ERR) << 8
                writeln!(out, "{}{}:", indent, el).ok();
                let e_gp = format!("%rgep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr [15 x i8], [15 x i8]* @STR_READFILE_ERR, i64 0, i64 0", indent, e_gp).ok();
                let e_pa = format!("%rfpa{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, e_pa, e_gp).ok();
                let e_sh = format!("%rfsh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = shl i64 {}, 8", indent, e_sh, e_pa).ok();
                let e_re = format!("%rfer{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = or i64 {}, 1", indent, e_re, e_sh).ok();
                writeln!(out, "{}br label %{}", indent, dl).ok();

                // Ok(contents) — packed Result: disc=0 low 8 bits,
                // payload = raw (already a Brief string pointer) << 8
                writeln!(out, "{}{}:", indent, ol).ok();
                let o_re = format!("%rfor{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = shl i64 {}, 8", indent, o_re, raw).ok();
                writeln!(out, "{}br label %{}", indent, dl).ok();

                writeln!(out, "{}{}:", indent, dl).ok();
                writeln!(out, "{}{} = phi i64 [ {}, %{} ], [ {}, %{} ]", indent, v, e_re, el, o_re, ol).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }
        Intrinsic::WriteFile => {
            // WriteFile(path: String, data: String) -> Bool
            // Brief strings are passed as boxed i64 (ptrtoint of header).
            let path_val = backend.emit_expr(out, &args[0], indent);
            let data_val = backend.emit_expr(out, &args[1], indent);
            let path_boxed = backend.adapt_to_i64(out, indent, &path_val);
            let data_boxed = backend.adapt_to_i64(out, indent, &data_val);
            let wf_ret = format!("%wfr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = call i64 @__write_file__(i64 {}, i64 {})", indent, wf_ret, path_boxed, data_boxed).ok();
            writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, v, wf_ret).ok();
            return TypedRegister { name: v.to_string(), ty: Type::Bool };
        }
        Intrinsic::Sleep => {
            // Sleep takes milliseconds, converts to seconds + nanoseconds for nanosleep
            let ms = backend.emit_expr(out, &args[0], indent);
            let micro = format!("%slmc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = mul i64 {}, 1000", indent, micro, ms.name).ok();
            let sec = format!("%slsc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = udiv i64 {}, 1000000", indent, sec, micro).ok();
            let usec = format!("%sluc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = urem i64 {}, 1000000", indent, usec, micro).ok();
            let nsec = format!("%slnec{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = mul i64 {}, 1000", indent, nsec, usec).ok();
            // Allocate and fill timespec
            let ts = format!("%slts{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let tsp = format!("%sltsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let tsnp = format!("%sltsn{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = alloca {{ i64, i64 }}, align 8", indent, ts).ok();
            writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 0", indent, tsp, ts).ok();
            writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 1", indent, tsnp, ts).ok();
            writeln!(out, "{}store i64 {}, ptr {}", indent, sec, tsp).ok();
            writeln!(out, "{}store i64 {}, ptr {}", indent, nsec, tsnp).ok();
            // Call nanosleep (ignore remainder)
            let rv = format!("%slrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = call i32 @nanosleep(ptr {}, ptr null)", indent, rv, ts).ok();
            // Return true (Bool)
            writeln!(out, "{}{} = add i64 0, 1 ; sleep done", indent, v).ok();
        }
        // ===== Phase A: Terminal (intrinsics.md D4) =====
        Intrinsic::TtyRawMode => {
            let arg = backend.emit_expr(out, &args[0], indent);
            let arg64 = backend.adapt_to_i64(out, indent, &arg);
            let raw = format!("%trm{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = call i64 @__tty_raw_mode__(i64 {})", indent, raw, arg64).ok();
            writeln!(out, "{}{} = trunc i64 {} to i1", indent, v, raw).ok();
            return TypedRegister { name: v.to_string(), ty: Type::Bool };
        }
        Intrinsic::TtySize => {
            let ws = format!("%ttywsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ws_bc = format!("%ttywsbc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%ttyrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let is_err = format!("%ttyie{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let z_l = format!("tty_sz_z{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let o_l = format!("tty_sz_o{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let e_l = format!("tty_sz_e{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let row_p = format!("%ttyrp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let col_p = format!("%ttycp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let row = format!("%ttyrw{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let col = format!("%ttycw{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let row64 = format!("%ttyr64{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let col64 = format!("%ttyc64{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let shifted = format!("%ttysh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let packed = format!("%ttypk{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = alloca {{ i16, i16, i16, i16 }}, align 2", indent, ws).ok();
            writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, ws_bc, ws).ok();
            writeln!(out, "{}{} = call i32 @ioctl(i32 1, i64 21523, ptr {})", indent, rv, ws_bc).ok();
            writeln!(out, "{}{} = icmp eq i32 {}, -1", indent, is_err, rv).ok();
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, is_err, z_l, o_l).ok();
            writeln!(out, "{}{}:", indent, z_l).ok();
            writeln!(out, "{}  br label %{}", indent, e_l).ok();
            writeln!(out, "{}{}:", indent, o_l).ok();
            writeln!(out, "{}{} = getelementptr {{ i16, i16, i16, i16 }}, ptr {}, i32 0, i32 0", indent, row_p, ws).ok();
            writeln!(out, "{}{} = getelementptr {{ i16, i16, i16, i16 }}, ptr {}, i32 0, i32 1", indent, col_p, ws).ok();
            writeln!(out, "{}{} = load i16, ptr {}", indent, row, row_p).ok();
            writeln!(out, "{}{} = load i16, ptr {}", indent, col, col_p).ok();
            writeln!(out, "{}{} = zext i16 {} to i64", indent, row64, row).ok();
            writeln!(out, "{}{} = zext i16 {} to i64", indent, col64, col).ok();
            // 2026-06-17: Pack as col * 10000 + row (C convention used by
            // officina: term_width = encoded/10000, term_height = encoded%10000)
            let mult = format!("%ttym{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = mul i64 {}, 10000", indent, mult, col64).ok();
            let packed = format!("%ttypk{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = add i64 {}, {}", indent, packed, mult, row64).ok();
            writeln!(out, "{}  br label %{}", indent, e_l).ok();
            writeln!(out, "{}{}:", indent, e_l).ok();
            writeln!(out, "{}{} = phi i64 [ 800024, %{} ], [ {}, %{} ]", indent, v, z_l, packed, o_l).ok();
        }
        Intrinsic::TtyReadKey => {
            let cbuf = format!("%trkcb{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%trkrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ok = format!("%trkok{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let err_l = format!("trk_err{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ok_l = format!("trk_ok{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let end_l = format!("trk_end{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let c = format!("%trkc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let tmp = format!("%trkt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = alloca i8, align 1", indent, cbuf).ok();
            writeln!(out, "{}{} = call i64 @read(i32 0, ptr {}, i64 1)", indent, rv, cbuf).ok();
            writeln!(out, "{}{} = icmp ne i64 {}, 1", indent, ok, rv).ok();
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, ok, err_l, ok_l).ok();
            writeln!(out, "{}{}:", indent, err_l).ok();
            writeln!(out, "{}  br label %{}", indent, end_l).ok();
            writeln!(out, "{}{}:", indent, ok_l).ok();
            writeln!(out, "{}{} = load i8, ptr {}", indent, c, cbuf).ok();
            writeln!(out, "{}{} = zext i8 {} to i32", indent, tmp, c).ok();
            writeln!(out, "{}  br label %{}", indent, end_l).ok();
            writeln!(out, "{}{}:", indent, end_l).ok();
            let phi_r = format!("%trkp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = phi i32 [ -1, %{} ], [ {}, %{} ]", indent, phi_r, err_l, tmp, ok_l).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, phi_r).ok();
            return TypedRegister { name: v.to_string(), ty: Type::Char };
        }
        Intrinsic::IoCtl => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let req = backend.emit_expr(out, &args[1], indent);
            let arg = backend.emit_expr(out, &args[2], indent);
            let fdt = format!("%iofdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ap = format!("%ioap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%iorv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, arg.name).ok();
            writeln!(out, "{}{} = call i32 @ioctl(i32 {}, i64 {}, ptr {})", indent, rv, fdt, req.name, ap).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::IsTty => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let fdt = format!("%istfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%istrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = call i32 @isatty(i32 {})", indent, rv, fdt).ok();
            writeln!(out, "{}{} = trunc i32 {} to i1", indent, v, rv).ok();
            return TypedRegister { name: v.to_string(), ty: Type::Bool };
        }
        // ===== Phase A: Process (intrinsics.md D5) =====
        Intrinsic::SpawnWithOutput => {
            let cmd = backend.emit_expr(out, &args[0], indent);
            let boxed = backend.adapt_to_i64(out, indent, &cmd);
            let raw = format!("%sp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            // brief_spawn_with_output takes i64 (Brief string ptr), returns i64
            writeln!(out, "{}{} = call i64 @__spawn_with_output__(i64 {})", indent, raw, boxed).ok();
            return TypedRegister { name: raw, ty: Type::Int };
        }
        Intrinsic::Spawn => {
            let cmd = backend.emit_expr(out, &args[0], indent);
            let sp = format!("%spwnsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let dp = format!("%spwndp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let cp = format!("%spwncp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let st = format!("%spwnst{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let neg = format!("%spwnng{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let wst = format!("%spwnws{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let andv = format!("%spwnan{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let val = format!("%spwnvl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let el = format!("spwn_er{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ol = format!("spwn_ok{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let _el = format!("spwn_en{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            // 2026-06-17: Direct libc — system() + WEXITSTATUS
            // system() returns int(i32): -1 on error, else waitpid status.
            // WEXITSTATUS = ((status & 0xff00) >> 8)
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, cmd.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
            writeln!(out, "{}{} = call i32 @system(ptr {})", indent, st, cp).ok();
            writeln!(out, "{}{} = icmp slt i32 {}, 0", indent, neg, st).ok();
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, neg, el, ol).ok();
            writeln!(out, "{}{}:", indent, el).ok();
            writeln!(out, "{}  br label %{}", indent, _el).ok();
            writeln!(out, "{}{}:", indent, ol).ok();
            writeln!(out, "{}{} = sext i32 {} to i64", indent, wst, st).ok();
            writeln!(out, "{}{} = and i64 {}, 0xff00", indent, andv, wst).ok();
            writeln!(out, "{}{} = lshr i64 {}, 8", indent, val, andv).ok();
            writeln!(out, "{}  br label %{}", indent, _el).ok();
            writeln!(out, "{}{}:", indent, _el).ok();
            writeln!(out, "{}{} = phi i64 [ -1, %{} ], [ {}, %{} ]", indent, v, el, val, ol).ok();
        }
        Intrinsic::Argv => {
            panic!("argv#() called at runtime — not supported in LLVM backend yet");
        }
        // ===== Phase B: Raw File I/O (intrinsics.md D2) =====
        Intrinsic::Open => {
            let path = backend.emit_expr(out, &args[0], indent);
            let flags = backend.emit_expr(out, &args[1], indent);
            let mode = backend.emit_expr(out, &args[2], indent);
            let sp = format!("%opsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let dp = format!("%opdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let cp = format!("%opcp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ft = format!("%opft{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let mt = format!("%opmt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%oprv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, mt, mode.name).ok();
            writeln!(out, "{}{} = call i32 @open(ptr {}, i32 {}, i32 {})", indent, rv, cp, ft, mt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::Close => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let fdt = format!("%cfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%crv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = call i32 @close(i32 {})", indent, rv, fdt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::Read => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let buf = backend.emit_expr(out, &args[1], indent);
            let count = backend.emit_expr(out, &args[2], indent);
            let fdt = format!("%rfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let bp = format!("%rbp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
            writeln!(out, "{}{} = call i64 @read(i32 {}, ptr {}, i64 {})", indent, v, fdt, bp, count.name).ok();
        }
        Intrinsic::Write => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let buf = backend.emit_expr(out, &args[1], indent);
            let count = backend.emit_expr(out, &args[2], indent);
            let fdt = format!("%wfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let bp = format!("%wbp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
            writeln!(out, "{}{} = call i64 @write(i32 {}, ptr {}, i64 {})", indent, v, fdt, bp, count.name).ok();
        }
        Intrinsic::LSeek => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let offset = backend.emit_expr(out, &args[1], indent);
            let whence = backend.emit_expr(out, &args[2], indent);
            let fdt = format!("%lfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let wt = format!("%lwt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, wt, whence.name).ok();
            writeln!(out, "{}{} = call i64 @lseek(i32 {}, i64 {}, i32 {})", indent, v, fdt, offset.name, wt).ok();
        }
        Intrinsic::PRead => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let buf = backend.emit_expr(out, &args[1], indent);
            let count = backend.emit_expr(out, &args[2], indent);
            let offset = backend.emit_expr(out, &args[3], indent);
            let fdt = format!("%prfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let bp = format!("%prbp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
            writeln!(out, "{}{} = call i64 @pread(i32 {}, ptr {}, i64 {}, i64 {})", indent, v, fdt, bp, count.name, offset.name).ok();
        }
        Intrinsic::PWrite => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let buf = backend.emit_expr(out, &args[1], indent);
            let count = backend.emit_expr(out, &args[2], indent);
            let offset = backend.emit_expr(out, &args[3], indent);
            let fdt = format!("%pwfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let bp = format!("%pwbp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
            writeln!(out, "{}{} = call i64 @pwrite(i32 {}, ptr {}, i64 {}, i64 {})", indent, v, fdt, bp, count.name, offset.name).ok();
        }
        Intrinsic::Stat => {
            let path = backend.emit_expr(out, &args[0], indent);
            let sp = format!("%stsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let dp = format!("%stdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let buf = format!("%stbuf{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%strv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, buf, dp).ok();
            let st = format!("%stst{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = alloca i8, i64 200, align 8", indent, st).ok();
            writeln!(out, "{}{} = call i32 @stat(ptr {}, ptr {})", indent, rv, buf, st).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::FStat => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let fdt = format!("%fsfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let st = format!("%fsst{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%fsrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = alloca i8, i64 200, align 8", indent, st).ok();
            writeln!(out, "{}{} = call i32 @fstat(i32 {}, ptr {})", indent, rv, fdt, st).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::FTruncate => {
            let path = backend.emit_expr(out, &args[0], indent);
            let len = backend.emit_expr(out, &args[1], indent);
            let sp = format!("%ttsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let dp = format!("%ttdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let cp = format!("%ttcp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%ttrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
            writeln!(out, "{}{} = call i32 @truncate(ptr {}, i64 {})", indent, rv, cp, len.name).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::FTruncate => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let len = backend.emit_expr(out, &args[1], indent);
            let fdt = format!("%ftfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%ftrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = call i32 @ftruncate(i32 {}, i64 {})", indent, rv, fdt, len.name).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::FSync => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let fdt = format!("%yfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%yrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = call i32 @fsync(i32 {})", indent, rv, fdt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::FDup => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let fdt = format!("%dfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%drv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = call i32 @dup(i32 {})", indent, rv, fdt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::FDup2 => {
            let old = backend.emit_expr(out, &args[0], indent);
            let newfd = backend.emit_expr(out, &args[1], indent);
            let ot = format!("%d2ot{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let nt = format!("%d2nt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%d2rv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ot, old.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, nt, newfd.name).ok();
            writeln!(out, "{}{} = call i32 @dup2(i32 {}, i32 {})", indent, rv, ot, nt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::FCntl => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let cmd = backend.emit_expr(out, &args[1], indent);
            let arg = backend.emit_expr(out, &args[2], indent);
            let fdt = format!("%cnfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ct = format!("%cnct{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%cnrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ct, cmd.name).ok();
            writeln!(out, "{}{} = call i32 @fcntl(i32 {}, i32 {}, i64 {})", indent, rv, fdt, ct, arg.name).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        // ===== Phase C: Filesystem (intrinsics.md D3) =====
        Intrinsic::MkDir => {
            let path = backend.emit_expr(out, &args[0], indent);
            let mode = backend.emit_expr(out, &args[1], indent);
            let sp = format!("%mksp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let dp = format!("%mkdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let cp = format!("%mkcp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let mt = format!("%mkmt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%mkrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, mt, mode.name).ok();
            writeln!(out, "{}{} = call i32 @mkdir(ptr {}, i32 {})", indent, rv, cp, mt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::RmDir => {
            let path = backend.emit_expr(out, &args[0], indent);
            let sp = format!("%rdsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let dp = format!("%rddp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let cp = format!("%rdcp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%rdrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
            writeln!(out, "{}{} = call i32 @rmdir(ptr {})", indent, rv, cp).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::Unlink => {
            let path = backend.emit_expr(out, &args[0], indent);
            let sp = format!("%ulsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let dp = format!("%uldp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let cp = format!("%ulcp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%ulrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
            writeln!(out, "{}{} = call i32 @unlink(ptr {})", indent, rv, cp).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::Rename => {
            let old = backend.emit_expr(out, &args[0], indent);
            let new = backend.emit_expr(out, &args[1], indent);
            let osp = format!("%rosp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let odp = format!("%rodp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let nsp = format!("%rnsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ndp = format!("%rndp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%rrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, osp, old.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, odp, osp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, nsp, new.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ndp, nsp).ok();
            let ocp = format!("%rocp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ncp = format!("%rncp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ocp, odp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ncp, ndp).ok();
            writeln!(out, "{}{} = call i32 @rename(ptr {}, ptr {})", indent, rv, ocp, ncp).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::SymLink => {
            let target = backend.emit_expr(out, &args[0], indent);
            let link = backend.emit_expr(out, &args[1], indent);
            let tsp = format!("%sytsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let tdp = format!("%sytdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let lsp = format!("%sylsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ldp = format!("%syldp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let tcp = format!("%sytcp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let lcp = format!("%sylcp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%syrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, tsp, target.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, tdp, tsp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, lsp, link.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ldp, lsp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, tcp, tdp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, lcp, ldp).ok();
            writeln!(out, "{}{} = call i32 @symlink(ptr {}, ptr {})", indent, rv, tcp, lcp).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::ReadLink => {
            let path = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}{} = call i64 @__readlink__(i64 {})", indent, v, path.name).ok();
        }
        Intrinsic::Link => {
            let old = backend.emit_expr(out, &args[0], indent);
            let new = backend.emit_expr(out, &args[1], indent);
            let osp = format!("%lkosp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let odp = format!("%lkodp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let nsp = format!("%lknsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ndp = format!("%lkndp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ocp = format!("%lkocp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ncp = format!("%lkncp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%lkrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, osp, old.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, odp, osp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, nsp, new.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ndp, nsp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ocp, odp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ncp, ndp).ok();
            writeln!(out, "{}{} = call i32 @link(ptr {}, ptr {})", indent, rv, ocp, ncp).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::GetCwd => {
            writeln!(out, "{}{} = call i64 @__getcwd__()", indent, v).ok();
        }
        Intrinsic::ChDir => {
            let path = backend.emit_expr(out, &args[0], indent);
            let sp = format!("%chdsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let dp = format!("%chddp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let cp = format!("%chdcp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%chdrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
            writeln!(out, "{}{} = call i32 @chdir(ptr {})", indent, rv, cp).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::ReadDir => {
            let path = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}{} = call i64 @__readdir__(i64 {})", indent, v, path.name).ok();
        }
        Intrinsic::ChMod => {
            let path = backend.emit_expr(out, &args[0], indent);
            let mode = backend.emit_expr(out, &args[1], indent);
            let sp = format!("%chmsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let dp = format!("%chmdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let cp = format!("%chmcp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let mt = format!("%chmmt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%chmrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, mt, mode.name).ok();
            writeln!(out, "{}{} = call i32 @chmod(ptr {}, i32 {})", indent, rv, cp, mt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::ChOwn => {
            let path = backend.emit_expr(out, &args[0], indent);
            let uid = backend.emit_expr(out, &args[1], indent);
            let gid = backend.emit_expr(out, &args[2], indent);
            let sp = format!("%chosp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let dp = format!("%chodp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let cp = format!("%chocp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ut = format!("%chout{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let gt = format!("%chogt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%chorv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ut, uid.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, gt, gid.name).ok();
            writeln!(out, "{}{} = call i32 @chown(ptr {}, i32 {}, i32 {})", indent, rv, cp, ut, gt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::UMask => {
            let mask = backend.emit_expr(out, &args[0], indent);
            let mt = format!("%ummt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%umrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, mt, mask.name).ok();
            writeln!(out, "{}{} = call i32 @umask(i32 {})", indent, rv, mt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::Access => {
            let path = backend.emit_expr(out, &args[0], indent);
            let mode = backend.emit_expr(out, &args[1], indent);
            let sp = format!("%acsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let dp = format!("%acdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let cp = format!("%accp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let mt = format!("%acmt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%acrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, path.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, mt, mode.name).ok();
            writeln!(out, "{}{} = call i32 @access(ptr {}, i32 {})", indent, rv, cp, mt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        // ===== Phase D: Memory (intrinsics.md D1) — Shim category =====
        Intrinsic::Mmap => {
            let addr = backend.emit_expr(out, &args[0], indent);
            let length = backend.emit_expr(out, &args[1], indent);
            let prot = backend.emit_expr(out, &args[2], indent);
            let flags = backend.emit_expr(out, &args[3], indent);
            let fd = backend.emit_expr(out, &args[4], indent);
            let offset = backend.emit_expr(out, &args[5], indent);
            let ap = format!("%mmap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let pt = format!("%mmpt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ft = format!("%mmft{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let fdt = format!("%mmfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ret_ptr = format!("%mmret{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, pt, prot.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = call ptr @mmap(ptr {}, i64 {}, i32 {}, i32 {}, i32 {}, i64 {})", indent, ret_ptr, ap, length.name, pt, ft, fdt, offset.name).ok();
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, ret_ptr).ok();
        }
        Intrinsic::MUnmap => {
            let addr = backend.emit_expr(out, &args[0], indent);
            let length = backend.emit_expr(out, &args[1], indent);
            let ap = format!("%mua{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%murv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
            writeln!(out, "{}{} = call i32 @munmap(ptr {}, i64 {})", indent, rv, ap, length.name).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::MProtect => {
            let addr = backend.emit_expr(out, &args[0], indent);
            let length = backend.emit_expr(out, &args[1], indent);
            let prot = backend.emit_expr(out, &args[2], indent);
            let ap = format!("%mpa{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let pt = format!("%mppt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%mprv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, pt, prot.name).ok();
            writeln!(out, "{}{} = call i32 @mprotect(ptr {}, i64 {}, i32 {})", indent, rv, ap, length.name, pt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::Brk => {
            let addr = backend.emit_expr(out, &args[0], indent);
            let ap = format!("%brap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%brrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
            writeln!(out, "{}{} = call i32 @brk(ptr {})", indent, rv, ap).ok();
            writeln!(out, "{}{} = sext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::MLock => {
            let addr = backend.emit_expr(out, &args[0], indent);
            let length = backend.emit_expr(out, &args[1], indent);
            let ap = format!("%mla{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%mlrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
            writeln!(out, "{}{} = call i32 @mlock(ptr {}, i64 {})", indent, rv, ap, length.name).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        // ===== Phase D: Synchronization (intrinsics.md D9) — Native category =====
        Intrinsic::AtomicLoad => {
            let addr = backend.emit_expr(out, &args[0], indent);
            let _order = backend.emit_expr(out, &args[1], indent); // order arg consumed for eval
            let ptr = format!("%aptr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr.name).ok();
            writeln!(out, "{}{} = load atomic i64, ptr {} acquire, align 8", indent, v, ptr).ok();
        }
        Intrinsic::AtomicStore => {
            let addr = backend.emit_expr(out, &args[0], indent);
            let val = backend.emit_expr(out, &args[1], indent);
            let _order = backend.emit_expr(out, &args[2], indent);
            let ptr = format!("%aptr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr.name).ok();
            writeln!(out, "{}store atomic i64 {}, ptr {} release, align 8", indent, val.name, ptr).ok();
            writeln!(out, "{}{} = add i64 undef, 0 ; atomic_store is void", indent, v).ok();
        }
        Intrinsic::AtomicCas => {
            let addr = backend.emit_expr(out, &args[0], indent);
            let expected = backend.emit_expr(out, &args[1], indent);
            let new = backend.emit_expr(out, &args[2], indent);
            let _order = backend.emit_expr(out, &args[3], indent);
            let ptr = format!("%aptr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let pair = format!("%apair{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr.name).ok();
            writeln!(out, "{}{} = cmpxchg ptr {}, i64 {}, i64 {} acquire", indent, pair, ptr, expected.name, new.name).ok();
            writeln!(out, "{}{} = extractvalue {{ i64, i1 }} {}, 0", indent, v, pair).ok();
        }
        Intrinsic::AtomicXchg => {
            let addr = backend.emit_expr(out, &args[0], indent);
            let val = backend.emit_expr(out, &args[1], indent);
            let _order = backend.emit_expr(out, &args[2], indent);
            let ptr = format!("%aptr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr.name).ok();
            writeln!(out, "{}{} = atomicrmw xchg ptr {}, i64 {} acquire", indent, v, ptr, val.name).ok();
        }
        Intrinsic::AtomicAdd => {
            let addr = backend.emit_expr(out, &args[0], indent);
            let val = backend.emit_expr(out, &args[1], indent);
            let _order = backend.emit_expr(out, &args[2], indent);
            let ptr = format!("%aptr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr.name).ok();
            writeln!(out, "{}{} = atomicrmw add ptr {}, i64 {} acquire", indent, v, ptr, val.name).ok();
        }
        Intrinsic::Fence => {
            let _order = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}fence acquire", indent).ok();
            writeln!(out, "{}{} = add i64 undef, 0 ; fence is void", indent, v).ok();
        }
        Intrinsic::Futex => {
            // Evaluate all arguments for side effects (futex is a real
            // syscall with observable behavior when implemented).
            let _uaddr = backend.emit_expr(out, &args[0], indent);
            let _op = backend.emit_expr(out, &args[1], indent);
            let _val = backend.emit_expr(out, &args[2], indent);
            let _timeout = backend.emit_expr(out, &args[3], indent);
            let _uaddr2 = backend.emit_expr(out, &args[4], indent);
            let _val3 = backend.emit_expr(out, &args[5], indent);
            // 2026-06-17: Inline stub — C brief_futex was already a
            // stub returning -1 (futex is Linux-specific, architecture-
            // dependent). A real implementation would use @syscall.
            writeln!(out, "{}{} = add i64 0, -1", indent, v).ok();
        }
        // ===== Phase E: IPC (intrinsics.md D11) — Shim =====
        Intrinsic::Pipe => {
            let fds = backend.emit_expr(out, &args[0], indent);
            let parr = format!("%pipearr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let prv = format!("%piperv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let p0 = format!("%pipep0{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let p1 = format!("%pipep1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let pf0 = format!("%pipef0{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let pf1 = format!("%pipef1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let zf0 = format!("%pipef0z{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let zf1 = format!("%pipef1z{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let dst1 = format!("%piped1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let bp = format!("%pipebp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = alloca [2 x i32], align 4", indent, parr).ok();
            writeln!(out, "{}{} = call i32 @pipe(ptr {})", indent, prv, parr).ok();
            writeln!(out, "{}{} = getelementptr [2 x i32], ptr {}, i64 0, i64 0", indent, p0, parr).ok();
            writeln!(out, "{}{} = getelementptr [2 x i32], ptr {}, i64 0, i64 1", indent, p1, parr).ok();
            writeln!(out, "{}{} = load i32, ptr {}", indent, pf0, p0).ok();
            writeln!(out, "{}{} = load i32, ptr {}", indent, pf1, p1).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, zf0, pf0).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, zf1, pf1).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, fds.name).ok();
            writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, zf0, bp).ok();
            writeln!(out, "{}{} = getelementptr i64, ptr {}, i64 1", indent, dst1, bp).ok();
            writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, zf1, dst1).ok();
            writeln!(out, "{}{} = sext i32 {} to i64", indent, v, prv).ok();
        }
        Intrinsic::ShmOpen => {
            let name = backend.emit_expr(out, &args[0], indent);
            let flags = backend.emit_expr(out, &args[1], indent);
            let mode = backend.emit_expr(out, &args[2], indent);
            let nsp = format!("%shnsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ndp = format!("%shndp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ncp = format!("%shncp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ft = format!("%shft{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let mt = format!("%shmt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%shrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, nsp, name.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ndp, nsp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ncp, ndp).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, mt, mode.name).ok();
            writeln!(out, "{}{} = call i32 @shm_open(ptr {}, i32 {}, i32 {})", indent, rv, ncp, ft, mt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::ShmUnlink => {
            let name = backend.emit_expr(out, &args[0], indent);
            let nsp = format!("%slnsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ndp = format!("%slndp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ncp = format!("%slncp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%slrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, nsp, name.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ndp, nsp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ncp, ndp).ok();
            writeln!(out, "{}{} = call i32 @shm_unlink(ptr {})", indent, rv, ncp).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::SemOpen => {
            let name = backend.emit_expr(out, &args[0], indent);
            let flags = backend.emit_expr(out, &args[1], indent);
            let mode = backend.emit_expr(out, &args[2], indent);
            let value = backend.emit_expr(out, &args[3], indent);
            let nsp = format!("%sonsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ndp = format!("%sondp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ncp = format!("%soncp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ft = format!("%sonft{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let mt = format!("%sonmt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let vt = format!("%sonvt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rp = format!("%sonrp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, nsp, name.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ndp, nsp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ncp, ndp).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, mt, mode.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, vt, value.name).ok();
            writeln!(out, "{}{} = call ptr @sem_open(ptr {}, i32 {}, i32 {}, i32 {})", indent, rp, ncp, ft, mt, vt).ok();
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, rp).ok();
        }
        Intrinsic::SemWait => {
            let sem = backend.emit_expr(out, &args[0], indent);
            let sp = format!("%swsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%swrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, sem.name).ok();
            writeln!(out, "{}{} = call i32 @sem_wait(ptr {})", indent, rv, sp).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::SemPost => {
            let sem = backend.emit_expr(out, &args[0], indent);
            let sp = format!("%spsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%sprv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, sem.name).ok();
            writeln!(out, "{}{} = call i32 @sem_post(ptr {})", indent, rv, sp).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        // ===== Phase F: Signals (intrinsics.md D8) — Shim =====
        Intrinsic::SigAction => {
            let signum = backend.emit_expr(out, &args[0], indent);
            let handler = backend.emit_expr(out, &args[1], indent);
            writeln!(out, "{}{} = call i64 @__sigaction__(i64 {}, i64 {})", indent, v, signum.name, handler.name).ok();
        }
        Intrinsic::SigProcMask => {
            let how = backend.emit_expr(out, &args[0], indent);
            let mask = backend.emit_expr(out, &args[1], indent);
            writeln!(out, "{}{} = call i64 @__sigprocmask__(i64 {}, i64 {})", indent, v, how.name, mask.name).ok();
        }
        Intrinsic::Kill => {
            let pid = backend.emit_expr(out, &args[0], indent);
            let sig = backend.emit_expr(out, &args[1], indent);
            let pt = format!("%kpt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let st = format!("%kst{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%krv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, pt, pid.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, st, sig.name).ok();
            writeln!(out, "{}{} = call i32 @kill(i32 {}, i32 {})", indent, rv, pt, st).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::SignalFd => {
            // 2026-06-17: Direct libc — alloca sigset_t + memset + signalfd.
            // The C shim ignored the mask arg and created an empty set.
            let _mask = backend.emit_expr(out, &args[0], indent);
            let set = format!("%sigfds{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let bc = format!("%sigfdbc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%sigfdrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = alloca [16 x i64], align 8", indent, set).ok();
            writeln!(out, "{}{} = bitcast [16 x i64]* {} to ptr", indent, bc, set).ok();
            writeln!(out, "{}call void @llvm.memset.p0i8.i64(ptr {}, i8 0, i64 128, i1 false)", indent, bc).ok();
            writeln!(out, "{}{} = call i32 @signalfd(i32 -1, ptr {}, i32 2048)", indent, rv, bc).ok();
            writeln!(out, "{}{} = sext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::TimerFdCreate => {
            // 2026-06-17: Direct libc — timerfd_create + alloca itimerspec + timerfd_settime.
            // itimerspec layout on x86_64: {i64,i64,i64,i64} = {interval_sec, interval_nsec,
            // value_sec, value_nsec}
            let hz = backend.emit_expr(out, &args[0], indent);
            let nsec = format!("%tfnsec{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let fd = format!("%tffd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let spec = format!("%tfspec{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let sp = format!("%tfspp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let is0 = format!("%tfis0{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let z_l = format!("tf_z{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let s_l = format!("tf_s{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let e_l = format!("tf_e{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%tfrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let phiv = format!("%tfphi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = udiv i64 {}, 1000000000", indent, nsec, hz.name).ok();
            writeln!(out, "{}{} = call i32 @timerfd_create(i32 1, i32 2048)", indent, fd).ok();
            writeln!(out, "{}{} = icmp sgt i64 {}, 0", indent, is0, hz.name).ok();
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, is0, s_l, z_l).ok();
            writeln!(out, "{}{}:", indent, z_l).ok();
            writeln!(out, "{}  br label %{}", indent, e_l).ok();
            writeln!(out, "{}{}:", indent, s_l).ok();
            writeln!(out, "{}{} = alloca {{ i64, i64, i64, i64 }}, align 8", indent, spec).ok();
            writeln!(out, "{}{} = bitcast ptr {} to ptr", indent, sp, spec).ok();
            writeln!(out, "{}call void @llvm.memset.p0i8.i64(ptr {}, i8 0, i64 32, i1 false)", indent, sp).ok();
            let iv_ns = format!("%tfivns{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let vl_ns = format!("%tfvlns{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = urem i64 {}, 1000000000", indent, iv_ns, hz.name).ok();
            writeln!(out, "{}{} = urem i64 {}, 1000000000", indent, vl_ns, hz.name).ok();
            let iv_off = format!("%tfiiv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let vl_off = format!("%tfivl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 8", indent, iv_off, sp).ok();
            writeln!(out, "{}{} = getelementptr i8, ptr {}, i64 24", indent, vl_off, sp).ok();
            writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, iv_ns, iv_off).ok();
            writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, vl_ns, vl_off).ok();
            writeln!(out, "{}{} = call i32 @timerfd_settime(i32 {}, i32 0, ptr {}, ptr null)", indent, rv, fd, sp).ok();
            writeln!(out, "{}  br label %{}", indent, e_l).ok();
            writeln!(out, "{}{}:", indent, e_l).ok();
            writeln!(out, "{}{} = phi i32 [ {}, %{} ], [ {}, %{} ]", indent, phiv, fd, z_l, rv, s_l).ok();
            writeln!(out, "{}{} = sext i32 {} to i64", indent, v, phiv).ok();
        }
        // ===== Phase G: Networking (intrinsics.md D10) — Shim =====
        Intrinsic::Socket => {
            let domain = backend.emit_expr(out, &args[0], indent);
            let sock_type = backend.emit_expr(out, &args[1], indent);
            let protocol = backend.emit_expr(out, &args[2], indent);
            let dt = format!("%sodt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let st = format!("%sost{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let pt = format!("%sopt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%sorv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, dt, domain.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, st, sock_type.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, pt, protocol.name).ok();
            writeln!(out, "{}{} = call i32 @socket(i32 {}, i32 {}, i32 {})", indent, rv, dt, st, pt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::Bind => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let addr = backend.emit_expr(out, &args[1], indent);
            let addrlen = backend.emit_expr(out, &args[2], indent);
            let fdt = format!("%bifdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ap = format!("%bia{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let alt = format!("%bialt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%birv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, alt, addrlen.name).ok();
            writeln!(out, "{}{} = call i32 @bind(i32 {}, ptr {}, i32 {})", indent, rv, fdt, ap, alt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::Listen => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let backlog = backend.emit_expr(out, &args[1], indent);
            let fdt = format!("%lifdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let bt = format!("%libt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%lirv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, bt, backlog.name).ok();
            writeln!(out, "{}{} = call i32 @listen(i32 {}, i32 {})", indent, rv, fdt, bt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::Accept => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let addr = backend.emit_expr(out, &args[1], indent);
            let addrlen = backend.emit_expr(out, &args[2], indent);
            let fdt = format!("%acfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ap = format!("%acap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let als = format!("%acals{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let alt = format!("%acalt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%acrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
            writeln!(out, "{}{} = alloca i32, align 4", indent, als).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, alt, addrlen.name).ok();
            writeln!(out, "{}store i32 {}, ptr {}, align 4", indent, alt, als).ok();
            writeln!(out, "{}{} = call i32 @accept(i32 {}, ptr {}, ptr {})", indent, rv, fdt, ap, als).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::Connect => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let addr = backend.emit_expr(out, &args[1], indent);
            let addrlen = backend.emit_expr(out, &args[2], indent);
            let fdt = format!("%cofdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ap = format!("%coap{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let alt = format!("%coalt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%corv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ap, addr.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, alt, addrlen.name).ok();
            writeln!(out, "{}{} = call i32 @connect(i32 {}, ptr {}, i32 {})", indent, rv, fdt, ap, alt).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::Send => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let buf = backend.emit_expr(out, &args[1], indent);
            let len = backend.emit_expr(out, &args[2], indent);
            let flags = backend.emit_expr(out, &args[3], indent);
            let fdt = format!("%sdfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let bp = format!("%sdbp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ft = format!("%sdft{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
            writeln!(out, "{}{} = call i64 @send(i32 {}, ptr {}, i64 {}, i32 {})", indent, v, fdt, bp, len.name, ft).ok();
        }
        Intrinsic::Recv => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let buf = backend.emit_expr(out, &args[1], indent);
            let len = backend.emit_expr(out, &args[2], indent);
            let flags = backend.emit_expr(out, &args[3], indent);
            let fdt = format!("%rcfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let bp = format!("%rcbp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ft = format!("%rcft{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
            writeln!(out, "{}{} = call i64 @recv(i32 {}, ptr {}, i64 {}, i32 {})", indent, v, fdt, bp, len.name, ft).ok();
        }
        Intrinsic::SendTo => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let buf = backend.emit_expr(out, &args[1], indent);
            let len = backend.emit_expr(out, &args[2], indent);
            let flags = backend.emit_expr(out, &args[3], indent);
            let dest_addr = backend.emit_expr(out, &args[4], indent);
            let addrlen = backend.emit_expr(out, &args[5], indent);
            let fdt = format!("%stofdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let bp = format!("%stobp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ft = format!("%stoft{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let da = format!("%stoda{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let alt = format!("%stoalt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, da, dest_addr.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, alt, addrlen.name).ok();
            writeln!(out, "{}{} = call i64 @sendto(i32 {}, ptr {}, i64 {}, i32 {}, ptr {}, i32 {})", indent, v, fdt, bp, len.name, ft, da, alt).ok();
        }
        Intrinsic::RecvFrom => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let buf = backend.emit_expr(out, &args[1], indent);
            let len = backend.emit_expr(out, &args[2], indent);
            let flags = backend.emit_expr(out, &args[3], indent);
            let src_addr = backend.emit_expr(out, &args[4], indent);
            let addrlen = backend.emit_expr(out, &args[5], indent);
            let fdt = format!("%rfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let bp = format!("%rbp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ft = format!("%rft{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let sa = format!("%rsa{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let als = format!("%rals{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let alt = format!("%ralt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, bp, buf.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ft, flags.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sa, src_addr.name).ok();
            writeln!(out, "{}{} = alloca i32, align 4", indent, als).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, alt, addrlen.name).ok();
            writeln!(out, "{}store i32 {}, ptr {}, align 4", indent, alt, als).ok();
            writeln!(out, "{}{} = call i64 @recvfrom(i32 {}, ptr {}, i64 {}, i32 {}, ptr {}, ptr {})", indent, v, fdt, bp, len.name, ft, sa, als).ok();
        }
        Intrinsic::SetSockOpt => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let level = backend.emit_expr(out, &args[1], indent);
            let opt = backend.emit_expr(out, &args[2], indent);
            let val = backend.emit_expr(out, &args[3], indent);
            let len = backend.emit_expr(out, &args[4], indent);
            let fdt = format!("%ssofdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let lt = format!("%ssolt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ot = format!("%ssoot{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let vp = format!("%ssovp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let lt2 = format!("%ssolt2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%ssorv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, lt, level.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ot, opt.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, vp, val.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, lt2, len.name).ok();
            writeln!(out, "{}{} = call i32 @setsockopt(i32 {}, i32 {}, i32 {}, ptr {}, i32 {})", indent, rv, fdt, lt, ot, vp, lt2).ok();
            writeln!(out, "{}{} = sext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::GetSockOpt => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let level = backend.emit_expr(out, &args[1], indent);
            let opt = backend.emit_expr(out, &args[2], indent);
            let val = backend.emit_expr(out, &args[3], indent);
            let len = backend.emit_expr(out, &args[4], indent);
            let fdt = format!("%gsofdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let lt = format!("%gsolt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ot = format!("%gsoot{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let vp = format!("%gsovp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ls = format!("%gsols{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let lt2 = format!("%gsolt2{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%gsorv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, lt, level.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ot, opt.name).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, vp, val.name).ok();
            writeln!(out, "{}{} = alloca i32, align 4", indent, ls).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, lt2, len.name).ok();
            writeln!(out, "{}store i32 {}, ptr {}, align 4", indent, lt2, ls).ok();
            writeln!(out, "{}{} = call i32 @getsockopt(i32 {}, i32 {}, i32 {}, ptr {}, ptr {})", indent, rv, fdt, lt, ot, vp, ls).ok();
            writeln!(out, "{}{} = sext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::Shutdown => {
            let fd = backend.emit_expr(out, &args[0], indent);
            let how = backend.emit_expr(out, &args[1], indent);
            let fdt = format!("%shfdt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ht = format!("%shht{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%shrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fdt, fd.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ht, how.name).ok();
            writeln!(out, "{}{} = call i32 @shutdown(i32 {}, i32 {})", indent, rv, fdt, ht).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::GetAddrInfo => {
            let node = backend.emit_expr(out, &args[0], indent);
            let service = backend.emit_expr(out, &args[1], indent);
            writeln!(out, "{}{} = call i64 @__getaddrinfo__(i64 {}, i64 {})", indent, v, node.name, service.name).ok();
        }
        // ===== Phase H: Everything Else (intrinsics.md D6, D7) — Shim =====
        Intrinsic::GetEnv => {
            let name = backend.emit_expr(out, &args[0], indent);
            let sp = format!("%gesp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let dp = format!("%gedp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let cp = format!("%gecp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rp = format!("%gerp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, name.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
            writeln!(out, "{}{} = call ptr @getenv(ptr {})", indent, rp, cp).ok();
            writeln!(out, "{}{} = ptrtoint ptr {} to i64", indent, v, rp).ok();
        }
        Intrinsic::SetEnv => {
            let name = backend.emit_expr(out, &args[0], indent);
            let val = backend.emit_expr(out, &args[1], indent);
            let nsp = format!("%senp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ndp = format!("%sendp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let vsp = format!("%sevp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let vdp = format!("%sevdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ncp = format!("%secnp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let vcp = format!("%secvp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%serv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, nsp, name.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, ndp, nsp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, vsp, val.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, vdp, vsp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ncp, ndp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, vcp, vdp).ok();
            writeln!(out, "{}{} = call i32 @setenv(ptr {}, ptr {}, i32 1)", indent, rv, ncp, vcp).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::UnsetEnv => {
            let name = backend.emit_expr(out, &args[0], indent);
            let sp = format!("%uensp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let dp = format!("%uendp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let cp = format!("%uencp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%uenrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, name.name).ok();
            writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
            writeln!(out, "{}{} = call i32 @unsetenv(ptr {})", indent, rv, cp).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::GetPid => {
            let rv = format!("%gpidrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = call i32 @getpid()", indent, rv).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::GetPPid => {
            let rv = format!("%gpprv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = call i32 @getppid()", indent, rv).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::ClockGetTime => {
            let clock_id = backend.emit_expr(out, &args[0], indent);
            let ci = format!("%cgtci{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ts = format!("%cgtts{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let _rv = format!("%cgtrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let sp = format!("%cgtsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let np = format!("%cgtnp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let sec = format!("%cgtsec{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let nsec = format!("%cgtnsec{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let mulv = format!("%cgtmul{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ci, clock_id.name).ok();
            writeln!(out, "{}{} = alloca {{ i64, i64 }}, align 8", indent, ts).ok();
            writeln!(out, "{}{} = call i32 @clock_gettime(i32 {}, ptr {})", indent, _rv, ci, ts).ok();
            writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 0", indent, sp, ts).ok();
            writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 1", indent, np, ts).ok();
            writeln!(out, "{}{} = load i64, ptr {}", indent, sec, sp).ok();
            writeln!(out, "{}{} = load i64, ptr {}", indent, nsec, np).ok();
            writeln!(out, "{}{} = mul i64 {}, 1000000000", indent, mulv, sec).ok();
            writeln!(out, "{}{} = add i64 {}, {}", indent, v, mulv, nsec).ok();
        }
        Intrinsic::NanoSleep => {
            let ns = backend.emit_expr(out, &args[0], indent);
            let sec = format!("%nnsec{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let nsec = format!("%nnnsec{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ts = format!("%nnts{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let tsp = format!("%nntsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let tsnp = format!("%nntsnp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rem = format!("%nnrem{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rv = format!("%nnrv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let zero_c = format!("%nnzc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let z_l = format!("nn_z{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let r_l = format!("nn_r{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let e_l = format!("nn_e{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rsp = format!("%nnrsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rnp = format!("%nnrnp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rsec = format!("%nnrsec{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rnsec = format!("%nnrnsec{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rmul = format!("%nnrmul{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let rns = format!("%nnrns{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = udiv i64 {}, 1000000000", indent, sec, ns.name).ok();
            writeln!(out, "{}{} = urem i64 {}, 1000000000", indent, nsec, ns.name).ok();
            writeln!(out, "{}{} = alloca {{ i64, i64 }}, align 8", indent, ts).ok();
            writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 0", indent, tsp, ts).ok();
            writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 1", indent, tsnp, ts).ok();
            writeln!(out, "{}store i64 {}, ptr {}", indent, sec, tsp).ok();
            writeln!(out, "{}store i64 {}, ptr {}", indent, nsec, tsnp).ok();
            writeln!(out, "{}{} = alloca {{ i64, i64 }}, align 8", indent, rem).ok();
            writeln!(out, "{}{} = call i32 @nanosleep(ptr {}, ptr {})", indent, rv, ts, rem).ok();
            writeln!(out, "{}{} = icmp eq i32 {}, 0", indent, zero_c, rv).ok();
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, zero_c, z_l, r_l).ok();
            writeln!(out, "{}{}:", indent, z_l).ok();
            writeln!(out, "{}  br label %{}", indent, e_l).ok();
            writeln!(out, "{}{}:", indent, r_l).ok();
            writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 0", indent, rsp, rem).ok();
            writeln!(out, "{}{} = getelementptr {{ i64, i64 }}, ptr {}, i32 0, i32 1", indent, rnp, rem).ok();
            writeln!(out, "{}{} = load i64, ptr {}", indent, rsec, rsp).ok();
            writeln!(out, "{}{} = load i64, ptr {}", indent, rnsec, rnp).ok();
            writeln!(out, "{}{} = mul i64 {}, 1000000000", indent, rmul, rsec).ok();
            writeln!(out, "{}{} = add i64 {}, {}", indent, rns, rmul, rnsec).ok();
            writeln!(out, "{}  br label %{}", indent, e_l).ok();
            writeln!(out, "{}{}:", indent, e_l).ok();
            writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, v, z_l, rns, r_l).ok();
        }
        Intrinsic::Sort => {
            if let Some(first) = args.first() {
                let list_val = backend.emit_expr(out, first, indent);
                let boxed = backend.adapt_to_i64(out, indent, &list_val);
                writeln!(out, "{}{} = call i64 @__sort_list__(i64 {})", indent, v, boxed).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }
        Intrinsic::Reverse => {
            if let Some(first) = args.first() {
                let list_val = backend.emit_expr(out, first, indent);
                let boxed = backend.adapt_to_i64(out, indent, &list_val);
                writeln!(out, "{}{} = call i64 @__reverse_list__(i64 {})", indent, v, boxed).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }
        Intrinsic::Range => {
            if let Some(first) = args.first() {
                let end_val = backend.emit_expr(out, first, indent);
                let boxed = backend.adapt_to_i64(out, indent, &end_val);
                writeln!(out, "{}{} = call i64 @__range__(i64 {})", indent, v, boxed).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }
        Intrinsic::TrimLeft => {
            if let Some(first) = args.first() {
                let s = backend.emit_expr(out, first, indent);
                let sp = format!("%tls{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let dp = format!("%tld{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let cp = format!("%tlc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, s).ok();
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                writeln!(out, "{}{} = call i64 @__trim_left__(ptr {})", indent, v, cp).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }
        Intrinsic::TrimRight => {
            if let Some(first) = args.first() {
                let s = backend.emit_expr(out, first, indent);
                let sp = format!("%trs{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let dp = format!("%trd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let cp = format!("%trc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, s).ok();
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                writeln!(out, "{}{} = call i64 @__trim_right__(ptr {})", indent, v, cp).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }
        Intrinsic::ToLower => {
            if let Some(first) = args.first() {
                let s = backend.emit_expr(out, first, indent);
                let sp = format!("%tlrs{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let dp = format!("%tlrd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let cp = format!("%tlrc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, s).ok();
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                writeln!(out, "{}{} = call i64 @__to_lower__(ptr {})", indent, v, cp).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }
        Intrinsic::ContainsAt => {
            if args.len() >= 3 {
                let haystack = backend.emit_expr(out, &args[0], indent);
                let needle = backend.emit_expr(out, &args[1], indent);
                let start = backend.emit_expr(out, &args[2], indent);
                let sp = format!("%cas{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let dp = format!("%cad{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let cp = format!("%cac{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let sp2 = format!("%cbs{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let dp2 = format!("%cbd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let cp2 = format!("%cbc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, haystack).ok();
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp2, needle).ok();
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp2, sp2).ok();
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp2, dp2).ok();
                writeln!(out, "{}{} = call i64 @__contains_at__(ptr {}, ptr {}, i64 {})", indent, v, cp, cp2, start).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }
        Intrinsic::FindFrom => {
            if args.len() >= 3 {
                let s = backend.emit_expr(out, &args[0], indent);
                let needle = backend.emit_expr(out, &args[1], indent);
                let start = backend.emit_expr(out, &args[2], indent);
                let sp = format!("%ffs{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let dp = format!("%ffd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let cp = format!("%ffc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let sp2 = format!("%fns{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let dp2 = format!("%fnd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let cp2 = format!("%fnc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, s).ok();
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp2, needle).ok();
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp2, sp2).ok();
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp2, dp2).ok();
                writeln!(out, "{}{} = call i64 @__find_from__(ptr {}, ptr {}, i64 {})", indent, v, cp, cp2, start).ok();
            } else {
                panic!("emit_expr: Intrinsic::FindFrom called with fewer than 3 arguments");
            }
        }
        Intrinsic::SplitN => {
            if args.len() >= 3 {
                let s = backend.emit_expr(out, &args[0], indent);
                let delim = backend.emit_expr(out, &args[1], indent);
                let n_val = backend.emit_expr(out, &args[2], indent);
                let sp = format!("%sps{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let dp = format!("%spd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let cp = format!("%spc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let sp2 = format!("%sds{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let dp2 = format!("%sdd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                let cp2 = format!("%sdc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp, s).ok();
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp, sp).ok();
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp, dp).ok();
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sp2, delim).ok();
                writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, dp2, sp2).ok();
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, cp2, dp2).ok();
                writeln!(out, "{}{} = call i64 @__splitn__(ptr {}, ptr {}, i64 {})", indent, v, cp, cp2, n_val).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }
        Intrinsic::IntToStr => {
            if let Some(first) = args.first() {
                let n = backend.emit_expr(out, first, indent);
                let boxed = backend.adapt_to_i64(out, indent, &n);
                writeln!(out, "{}{} = call i64 @__int_to_str__(i64 {})", indent, v, boxed).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }
        Intrinsic::Strlen => {
            if let Some(first) = args.first() {
                let ptr = backend.emit_expr(out, first, indent);
                let ptr_name = backend.adapt_to_i64(out, indent, &ptr);
                let unbox = format!("%pstr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, unbox, ptr_name).ok();
                writeln!(out, "{}{} = call i64 @strlen(ptr {})", indent, v, unbox).ok();
            } else {
                panic!("emit_expr: intrinsic called without required arguments");
            }
        }

        // Benchmark intrinsics (2026-06-16) — direct libc, no brief_rt.c shims
        Intrinsic::PrintInt => {
            let n = backend.emit_expr(out, &args[0], indent);
            let so = format!("%pso{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let fmt = format!("%pfi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let pi = format!("%ppi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so).ok();
            writeln!(out, "{}{} = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0", indent, fmt).ok();
            writeln!(out, "{}{} = call i32 (ptr, ptr, ...) @fprintf(ptr {}, ptr {}, i64 {})",
                indent, pi, so, fmt, n).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, pi).ok();
        }
        Intrinsic::PutChar => {
            let c = backend.emit_expr(out, &args[0], indent);
            let ct = format!("%pct{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let pc = format!("%ppc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let so = format!("%pso{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, ct, c).ok();
            writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so).ok();
            writeln!(out, "{}{} = call i32 @fputc(i32 {}, ptr {})",
                indent, v, ct, so).ok();
        }
        Intrinsic::PrintFloat => {
            let d = backend.emit_expr(out, &args[0], indent);
            let fl = backend.ensure_float_reg(out, indent, &d);
            let fd = format!("%pfd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let so = format!("%pso{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let fmt = format!("%pff{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let pf = format!("%ppf{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            // 2026-06-29: Float64 is already double, skip fpext
            if d.ty == Type::Float64 {
                writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so).ok();
                writeln!(out, "{}{} = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0", indent, fmt).ok();
                writeln!(out, "{}{} = call i32 (ptr, ptr, ...) @fprintf(ptr {}, ptr {}, double {})",
                    indent, pf, so, fmt, fl).ok();
            } else {
                writeln!(out, "{}{} = fpext float {} to double", indent, fd, fl).ok();
                writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so).ok();
                writeln!(out, "{}{} = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0", indent, fmt).ok();
                writeln!(out, "{}{} = call i32 (ptr, ptr, ...) @fprintf(ptr {}, ptr {}, double {})",
                    indent, pf, so, fmt, fd).ok();
            }
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, pf).ok();
        }
        Intrinsic::GetEnvInt => {
            let name = backend.emit_expr(out, &args[0], indent);
            // Brief String value is i64 (ptrtoint of struct ptr).
            // The struct has layout { ptr_to_data: i64, length: i64, data: [N x i8] }.
            // Load the first field (ptr_to_data) to get the actual data pointer.
            let sptr = format!("%gsr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let sp = format!("%gsp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let data_ptr = format!("%gdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let str_ptr = format!("%gnp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let gv = format!("%gnv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let isnull = format!("%gnvl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let nul_l = format!("genv_nul{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let ok_l = format!("genv_ok{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let after_l = format!("genv_af{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, sptr, name).ok();
            writeln!(out, "{}{} = bitcast ptr {} to i64*", indent, sp, sptr).ok();
            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, data_ptr, sp).ok();
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, str_ptr, data_ptr).ok();
            writeln!(out, "{}{} = call ptr @getenv(ptr {})", indent, gv, str_ptr).ok();
            writeln!(out, "{}{} = icmp eq ptr {}, null", indent, isnull, gv).ok();
            writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, isnull, nul_l, ok_l).ok();
            writeln!(out, "{}{}:", indent, nul_l).ok();
            writeln!(out, "{}  br label %{}", indent, after_l).ok();
            writeln!(out, "{}{}:", indent, ok_l).ok();
            let av = format!("%gav{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = call i64 @atol(ptr {})", indent, av, gv).ok();
            writeln!(out, "{}  br label %{}", indent, after_l).ok();
            writeln!(out, "{}{}:", indent, after_l).ok();
            writeln!(out, "{}{} = phi i64 [ 0, %{} ], [ {}, %{} ]",
                indent, v, nul_l, av, ok_l).ok();
        }
        Intrinsic::SetStdoutBuf => {
            let mode = backend.emit_expr(out, &args[0], indent);
            let mt = format!("%gbm{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let so = format!("%gbso{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, mt, mode).ok();
            writeln!(out, "{}{} = load ptr, ptr @stdout", indent, so).ok();
            writeln!(out, "{}{} = call i32 @setvbuf(ptr {}, ptr null, i32 {}, i64 0)",
                indent, v, so, mt).ok();
        }
        Intrinsic::Compile => {
            // compile#() is compile-time only — should never reach LLVM backend
            panic!("compile#() called at runtime — this is a compiler bug");
        }
        Intrinsic::MacroError => {
            panic!("error#() called at runtime — this is a compiler bug");
        }
        Intrinsic::MacroWarn => {
            panic!("warn#() called at runtime — this is a compiler bug");
        }
        Intrinsic::MacroGenSym => {
            panic!("gensym#() called at runtime — this is a compiler bug");
        }
        Intrinsic::EmitFile => {
            panic!("emit_file#() called at runtime — this is a compiler bug");
        }
        // GPU compute intrinsics (2026-06-18)
        // CPU fallback: emit C runtime calls that return sensible defaults.
        Intrinsic::GetGlobalId => {
            let dim = if !args.is_empty() {
                backend.emit_expr(out, &args[0], indent).name.clone()
            } else { "0".to_string() };
            writeln!(out, "{}  {} = call i64 @__get_global_id(i32 {})", indent, v, dim).ok();
        }
        Intrinsic::GetLocalId => {
            let dim = if !args.is_empty() {
                backend.emit_expr(out, &args[0], indent).name.clone()
            } else { "0".to_string() };
            writeln!(out, "{}  {} = call i64 @__get_local_id(i32 {})", indent, v, dim).ok();
        }
        Intrinsic::GetGroupId => {
            let dim = if !args.is_empty() {
                backend.emit_expr(out, &args[0], indent).name.clone()
            } else { "0".to_string() };
            writeln!(out, "{}  {} = call i64 @__get_group_id(i32 {})", indent, v, dim).ok();
        }
        Intrinsic::GetNumGroups => {
            let dim = if !args.is_empty() {
                backend.emit_expr(out, &args[0], indent).name.clone()
            } else { "0".to_string() };
            writeln!(out, "{}  {} = call i64 @__get_num_groups(i32 {})", indent, v, dim).ok();
        }
        Intrinsic::SubGroupBarrier => {
            writeln!(out, "{}  call void @__barrier__()", indent).ok();
            writeln!(out, "{}  {} = add i64 0, 1 ; barrier returns true", indent, v).ok();
        }
        // ===== D12: Random / Entropy (2026-06-19) =====
        Intrinsic::Errno => {
            writeln!(out, "{}  {} = call i64 @__errno__()", indent, v).ok();
        }
        Intrinsic::GetRandom => {
            let buf = backend.emit_expr(out, &args[0], indent);
            let len = backend.emit_expr(out, &args[1], indent);
            let flags = backend.emit_expr(out, &args[2], indent);
            writeln!(out, "{}  {} = call i64 @__getrandom__(i64 {}, i64 {}, i64 {})",
                indent, v, buf.name, len.name, flags.name).ok();
        }
        // ===== D13: System Info (2026-06-19) =====
        Intrinsic::Uname => {
            writeln!(out, "{}  {} = call i64 @__uname__()", indent, v).ok();
        }
        Intrinsic::PageSize => {
            writeln!(out, "{}  {} = call i64 @sysconf(i32 29) ; _SC_PAGESIZE", indent, v).ok();
        }
        Intrinsic::CpuCount => {
            writeln!(out, "{}  {} = call i64 @sysconf(i32 84) ; _SC_NPROCESSORS_ONLN", indent, v).ok();
        }
        Intrinsic::Hostname => {
            writeln!(out, "{}  {} = call i64 @__hostname__()", indent, v).ok();
        }
        Intrinsic::StrError => {
            let errnum = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__strerror__(i64 {})", indent, v, errnum.name).ok();
        }
        Intrinsic::StrSignal => {
            let signum = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__strsignal__(i64 {})", indent, v, signum.name).ok();
        }
        Intrinsic::RealPath => {
            let path = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__realpath__(i64 {})", indent, v, path.name).ok();
        }
        // ===== D14: Debugging (2026-06-19) =====
        Intrinsic::Abort => {
            writeln!(out, "{}  call void @abort()", indent).ok();
            writeln!(out, "{}  call void @llvm.trap()", indent).ok();
            return TypedRegister { name: "_".to_string(), ty: Type::Void };
        }
        Intrinsic::Backtrace => {
            writeln!(out, "{}  {} = call i64 @__backtrace__()", indent, v).ok();
        }
        // ===== D15: Scheduling (2026-06-19) =====
        Intrinsic::SchedYield => {
            let rv = format!("%sy{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = call i32 @sched_yield()", indent, rv).ok();
            writeln!(out, "{}{} = sext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::GetPriority => {
            let which = backend.emit_expr(out, &args[0], indent);
            let who = backend.emit_expr(out, &args[1], indent);
            let wi = format!("%gwi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let wo = format!("%gwo{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, wi, which.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, wo, who.name).ok();
            let rv = format!("%gpr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = call i32 @getpriority(i32 {}, i32 {})", indent, rv, wi, wo).ok();
            writeln!(out, "{}{} = sext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::SetPriority => {
            let which = backend.emit_expr(out, &args[0], indent);
            let who = backend.emit_expr(out, &args[1], indent);
            let prio = backend.emit_expr(out, &args[2], indent);
            let wi = format!("%swi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let wo = format!("%swo{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            let wp = format!("%swp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, wi, which.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, wo, who.name).ok();
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, wp, prio.name).ok();
            let rv = format!("%spr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = call i32 @setpriority(i32 {}, i32 {}, i32 {})", indent, rv, wi, wo, wp).ok();
            writeln!(out, "{}{} = sext i32 {} to i64", indent, v, rv).ok();
        }
        // ===== D16: User / Group (2026-06-19) =====
        Intrinsic::GetUid => {
            let rv = format!("%guid{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = call i32 @getuid()", indent, rv).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::GetEUid => {
            let rv = format!("%geuid{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = call i32 @geteuid()", indent, rv).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::GetGid => {
            let rv = format!("%ggid{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = call i32 @getgid()", indent, rv).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::GetEGid => {
            let rv = format!("%gegid{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = call i32 @getegid()", indent, rv).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, rv).ok();
        }
        Intrinsic::GetPwUid => {
            let uid = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__getpwuid__(i64 {})", indent, v, uid.name).ok();
        }
        Intrinsic::GetGrGid => {
            let gid = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__getgrgid__(i64 {})", indent, v, gid.name).ok();
        }
        // ===== D17: Threading (2026-06-19) =====
        Intrinsic::ThreadCreate => {
            let fn_ptr = backend.emit_expr(out, &args[0], indent);
            let arg = backend.emit_expr(out, &args[1], indent);
            writeln!(out, "{}  {} = call i64 @__thread_create__(i64 {}, i64 {})",
                indent, v, fn_ptr.name, arg.name).ok();
        }
        Intrinsic::ThreadJoin => {
            let thread = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__thread_join__(i64 {})", indent, v, thread.name).ok();
        }
        Intrinsic::ThreadExit => {
            let code = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  call void @__thread_exit__(i64 {})", indent, code.name).ok();
            writeln!(out, "{}  {} = add i64 undef, 0 ; thread_exit is noreturn", indent, v).ok();
        }
        Intrinsic::MutexLock => {
            let mptr = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__mutex_lock__(i64 {})", indent, v, mptr.name).ok();
        }
        Intrinsic::MutexUnlock => {
            let mptr = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__mutex_unlock__(i64 {})", indent, v, mptr.name).ok();
        }
        Intrinsic::CondvarWait => {
            let cptr = backend.emit_expr(out, &args[0], indent);
            let mptr = backend.emit_expr(out, &args[1], indent);
            writeln!(out, "{}  {} = call i64 @__condvar_wait__(i64 {}, i64 {})",
                indent, v, cptr.name, mptr.name).ok();
        }
        Intrinsic::CondvarSignal => {
            let cptr = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__condvar_signal__(i64 {})", indent, v, cptr.name).ok();
        }
        Intrinsic::CondvarBroadcast => {
            let cptr = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__condvar_broadcast__(i64 {})", indent, v, cptr.name).ok();
        }
        // ===== D18: Resource Limits (2026-06-19) =====
        Intrinsic::GetRlimit => {
            let resource = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__getrlimit__(i64 {})", indent, v, resource.name).ok();
        }
        Intrinsic::SetRlimit => {
            let resource = backend.emit_expr(out, &args[0], indent);
            let packed = backend.emit_expr(out, &args[1], indent);
            writeln!(out, "{}  {} = call i64 @__setrlimit__(i64 {}, i64 {})",
                indent, v, resource.name, packed.name).ok();
        }
        // ===== Extra intrinsics (2026-06-19) =====
        Intrinsic::MkStemp => {
            let template = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__mkstemp__(i64 {})", indent, v, template.name).ok();
        }
        Intrinsic::MkDtemp => {
            let template = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__mkdtemp__(i64 {})", indent, v, template.name).ok();
        }
        Intrinsic::DlOpen => {
            let filename = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__dlopen__(i64 {})", indent, v, filename.name).ok();
        }
        Intrinsic::DlSym => {
            let handle = backend.emit_expr(out, &args[0], indent);
            let symbol = backend.emit_expr(out, &args[1], indent);
            writeln!(out, "{}  {} = call i64 @__dlsym__(i64 {}, i64 {})",
                indent, v, handle.name, symbol.name).ok();
        }
        Intrinsic::DlClose => {
            let handle = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__dlclose__(i64 {})", indent, v, handle.name).ok();
        }
        Intrinsic::TtyName => {
            let fd = backend.emit_expr(out, &args[0], indent);
            writeln!(out, "{}  {} = call i64 @__ttyname__(i64 {})", indent, v, fd.name).ok();
        }
        Intrinsic::Halt => {
            // CPU halt: WFI on ARM, HLT on x86, WFI on RISC-V
            // The target triple determines the instruction.
            writeln!(out, "{}call void asm sideeffect \"wfi\", \"\"()", indent).ok();
            writeln!(out, "{}{} = add i64 undef, 0 ; halt is void", indent, v).ok();
        }
        Intrinsic::VolatileLoad => {
            let addr = backend.emit_expr(out, &args[0], indent);
            // Safety net: typechecker should have caught non-Ptr types
            debug_assert!(
                matches!(&addr.ty, Type::Applied(n, _) if n == "Ptr"),
                "volatile_load# expected Ptr<T>, got {:?}", addr.ty
            );
            let ptr = format!("%vlptr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr.name).ok();
            // Extract T from Ptr<T> argument type
            let t = if let Type::Applied(name, inners) = &addr.ty {
                if name == "Ptr" {
                    inners.first().cloned().unwrap_or(Type::Int)
                } else {
                    Type::Int
                }
            } else {
                Type::Int
            };
            let llvm_t = backend.llvm_type(&t).to_string();
            let raw = format!("%vlraw{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = load volatile {}, ptr {}", indent, raw, llvm_t, ptr).ok();
            // Box result to i64 if needed
            match t {
                Type::Bool => {
                    writeln!(out, "{}{} = zext {} {} to i64", indent, v, llvm_t, raw).ok();
                }
                Type::Char => {
                    writeln!(out, "{}{} = zext i32 {} to i64", indent, v, raw).ok();
                }
                Type::Float => {
                    let bi = format!("%vlbi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, raw).ok();
                    writeln!(out, "{}{} = zext i32 {} to i64", indent, v, bi).ok();
                }
                _ => {
                    writeln!(out, "{}{} = add i64 0, {}", indent, v, raw).ok();
                }
            }
            return TypedRegister { name: v.to_string(), ty: t };
        }
        Intrinsic::VolatileStore => {
            let addr = backend.emit_expr(out, &args[0], indent);
            let val = backend.emit_expr(out, &args[1], indent);
            // Safety net: typechecker should have caught non-Ptr types
            debug_assert!(
                matches!(&addr.ty, Type::Applied(n, _) if n == "Ptr"),
                "volatile_store# expected Ptr<T>, got {:?}", addr.ty
            );
            let ptr = format!("%vsptr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to ptr", indent, ptr, addr.name).ok();
            // Extract T from Ptr<T> argument type
            let t = if let Type::Applied(name, inners) = &addr.ty {
                if name == "Ptr" {
                    inners.first().cloned().unwrap_or(Type::Int)
                } else {
                    Type::Int
                }
            } else {
                Type::Int
            };
            let llvm_t = backend.llvm_type(&t).to_string();
            // Unbox val from i64 to native type T
            let native_val = match t {
                Type::Bool => {
                    let tr = format!("%vstr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = trunc i64 {} to {}", indent, tr, val.name, llvm_t).ok();
                    tr
                }
                Type::Char => {
                    let tr = format!("%vstr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, val.name).ok();
                    tr
                }
                Type::Float => {
                    let tr = format!("%vstr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    let bi = format!("%vsbi{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, val.name).ok();
                    writeln!(out, "{}{} = bitcast i32 {} to float", indent, bi, tr).ok();
                    bi
                }
                _ => {
                    val.name.clone()
                }
            };
            writeln!(out, "{}store volatile {} {}, ptr {}", indent, llvm_t, native_val, ptr).ok();
            writeln!(out, "{}{} = add i64 0, 1 ; volatile_store success", indent, v).ok();
            return TypedRegister { name: v.to_string(), ty: Type::Bool };
        }
        // ── Ring Buffer intrinsics (2026-07-01) ──────────────────────
        //
        // Ring buffer layout (boxed as i64 handle → inttoptr → i64*):
        //   offset 0: data  (i64 — ptrtoint of the element buffer)
        //   offset 1: head  (i64 — read index)
        //   offset 2: tail  (i64 — write index)
        //   offset 3: mask  (i64 — capacity-1, power of 2)
        //
        // RingPush(handle, value): writes value at buf[tail], increments
        //   tail with wrap (tail = (tail+1) & mask), returns handle unchanged.
        //   ~10 instructions, no alloc, no memcpy.
        //
        // RingPop(handle): reads buf[head] if head != tail, increments
        //   head with wrap, returns value (or 0 if empty).
        //   ~15 instructions, single empty check branch.
        //
        // Compared to arena-based List push/pop: ~40+ instructions,
        // 2 memcpy calls, 1 arena alloc.
        //
        Intrinsic::RingPush => {
            let handle = backend.emit_expr(out, &args[0], indent);
            let value = if args.len() > 1 { backend.emit_expr(out, &args[1], indent) } else { return TypedRegister { name: handle.name.clone(), ty: Type::Int }; };
            let h_ptr = format!("%rhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, h_ptr, handle.name).ok();
            let tail_gep = format!("%rtg{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, tail_gep, h_ptr).ok();
            let tail = format!("%rtl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, tail, tail_gep).ok();
            let mask_gep = format!("%rmg{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 3", indent, mask_gep, h_ptr).ok();
            let mask = format!("%rmk{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, mask, mask_gep).ok();
            let buf_gep = format!("%rbg{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, buf_gep, h_ptr).ok();
            let buf_raw = format!("%rbr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, buf_raw, buf_gep).ok();
            let buf_ptr = format!("%rbp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, buf_ptr, buf_raw).ok();
            let slot = format!("%rsl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, slot, buf_ptr, tail).ok();
            writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, value.name, slot).ok();
            let tail_next = format!("%rtn{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = add i64 {}, 1", indent, tail_next, tail).ok();
            let new_tail = format!("%rnt{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = and i64 {}, {}", indent, new_tail, tail_next, mask).ok();
            writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, new_tail, tail_gep).ok();
            return TypedRegister { name: handle.name.clone(), ty: Type::Int };
        }
        Intrinsic::RingPop => {
            // Unbox handle → load head, tail, mask, buf → load value at head
            // → select result (0 if empty, loaded value otherwise)
            // → advance head with wrap (store head if not empty)
            // No explicit branch needed — select handles both paths.
            let handle = backend.emit_expr(out, &args[0], indent);
            let h_ptr = format!("%rhp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, h_ptr, handle.name).ok();
            let head_gep = format!("%rhg{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, head_gep, h_ptr).ok();
            let head = format!("%rhd{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, head, head_gep).ok();
            let tail_gep = format!("%rtg{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, tail_gep, h_ptr).ok();
            let tail = format!("%rtl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, tail, tail_gep).ok();
            let mask_gep = format!("%rmg{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 3", indent, mask_gep, h_ptr).ok();
            let mask = format!("%rmk{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, mask, mask_gep).ok();
            let buf_gep = format!("%rbg{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, buf_gep, h_ptr).ok();
            let buf_raw = format!("%rbr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, buf_raw, buf_gep).ok();
            let buf_ptr = format!("%rbp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, buf_ptr, buf_raw).ok();
            let slot = format!("%rsl{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, slot, buf_ptr, head).ok();
            let val = format!("%rva{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, val, slot).ok();
            let empty = format!("%rem{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, empty, head, tail).ok();
            let result = format!("%rrr{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = select i1 {}, i64 0, i64 {}", indent, result, empty, val).ok();
            let head_next = format!("%rhn{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = add i64 {}, 1", indent, head_next, head).ok();
            let wrapped = format!("%rwrapped{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = and i64 {}, {}", indent, wrapped, head_next, mask).ok();
            let new_head = format!("%rnh{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
            writeln!(out, "{}{} = select i1 {}, i64 {}, i64 {}", indent, new_head, empty, head, wrapped).ok();
            writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, new_head, head_gep).ok();
            return TypedRegister { name: result, ty: Type::Int };
        }
        Intrinsic::UserDefined(name) => {
            // Extract return and param type info before any mutable borrows.
            // Clone the inop declaration to avoid borrow conflicts with emit_expr.
            let inop_clone = backend.ctx.inop_decls.get(name).cloned();
            let ret_ty = inop_clone.as_ref().map_or("i64", |d| {
                if d.outputs.iter().any(|t| {
                    let resolved = backend.resolve_bild_type(t);
                    matches!(resolved, Type::Float)
                }) {
                    "float"
                } else {
                    "i64"
                }
            });
            let param_tys: Vec<String> = inop_clone.as_ref().map_or_else(Vec::new, |d| {
                d.params.iter().map(|(_, t)| {
                    let resolved = backend.resolve_bild_type(t);
                    backend.llvm_type(&resolved).to_string()
                }).collect()
            });
            // Pre-evaluate all arguments before emitting the call,
            // so argument computation code appears before the call instruction.
            let mut arg_regs = Vec::new();
            for arg in args {
                let r = backend.emit_expr(out, arg, indent);
                arg_regs.push(r.name.clone());
            }
            // Detect multi-output: outputs.len() > 1
            let has_state_access = inop_clone.as_ref()
                .map(|d| d.has_state_access).unwrap_or(false);
            let is_multi = inop_clone.as_ref()
                .map(|d| d.outputs.len() > 1).unwrap_or(false);

            if is_multi {
                // Multi-output call: emit struct-return call, then
                // extract values into a boxed tuple matching Brief's format.
                let count = inop_clone.as_ref().map(|d| d.outputs.len()).unwrap_or(2);
                let struct_ty = (0..count).map(|_| "i64").collect::<Vec<_>>().join(", ");
                let struct_ty = format!("{{ {} }}", struct_ty);

                // Emit the call
                let call_reg = format!("%mc{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                write!(out, "{}{} = call {} @{}(", indent, call_reg, struct_ty, name).ok();
                if has_state_access {
                    write!(out, " ptr %state").ok();
                }
                for (i, rn) in arg_regs.iter().enumerate() {
                    let native_ty = param_tys.get(i).map(|s| s.as_str()).unwrap_or("i64");
                    if has_state_access || i > 0 {
                        write!(out, ", {} {}", native_ty, rn).ok();
                    } else {
                        write!(out, "{} {}", native_ty, rn).ok();
                    }
                }
                writeln!(out, ")").ok();

                // Allocate boxed tuple: [header_ptr, len, elem0, elem1, ...]
                let total = count as i64 + 2;
                let ai = format!("%mai{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = alloca i64, i64 {}", indent, ai, total).ok();
                let dp_ptr = format!("%mdp{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp_ptr, ai).ok();
                let dp_val = format!("%mdv{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, dp_val, dp_ptr).ok();
                let s0 = format!("%ms0{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, s0, ai).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, dp_val, s0).ok();
                let s1 = format!("%ms1{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, s1, ai).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, count as i64, s1).ok();

                for i in 0..count {
                    let ev = format!("%mev{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = extractvalue {} {}, {}", indent, ev, struct_ty, call_reg, i).ok();
                    let ep = format!("%mep{}", backend.fun.txn_counter); backend.fun.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, ai, (i as i64) + 2).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, ev, ep).ok();
                }

                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, ai).ok();
            } else {
                // Emit the call with native types for each parameter
                write!(out, "{}{} = call {} @{}(", indent, v, ret_ty, name).ok();
                if has_state_access {
                    write!(out, "ptr %state").ok();
                }
                for (i, rn) in arg_regs.iter().enumerate() {
                    let native_ty = param_tys.get(i).map(|s| s.as_str()).unwrap_or("i64");
                    if has_state_access || i > 0 {
                        write!(out, ", {} {}", native_ty, rn).ok();
                    } else {
                        write!(out, "{} {}", native_ty, rn).ok();
                    }
                }
                writeln!(out, ")").ok();
            }
        }
    }
    return TypedRegister { name: v.to_string(), ty: Type::Int };
}