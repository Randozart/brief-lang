use crate::ast::{Expr, Statement, TopLevel, Type};
use crate::backend::llvm::{float_to_llvm_hex, LlvmBackend, TypedRegister};
use std::fmt::Write;

impl LlvmBackend {
    /// Check if any modifier has the given name and extract its export name.
    /// Returns Some(export_name) if #export or #export("name") was found.
    pub fn get_export_name(modifiers: &[crate::ast::Hashtag]) -> Option<String> {
        for tag in modifiers {
            if tag.name == "export" {
                let export_name = tag.value.clone().unwrap_or_else(|| tag.name.clone());
                if export_name == "export" {
                    return None;
                }
                return Some(export_name);
            }
        }
        None
    }

    /// Emit cleanup calls for all local variables whose type has an
    /// OnExit foreign destructor. Called at scope exit points.
    fn emit_on_exit_cleanup(&mut self, out: &mut String, indent: &str) {
        let Some(ref universe) = self.type_universe else { return };
        for (name, ty) in &self.let_binding_types {
            let type_name = match ty {
                crate::ast::Type::Custom(n) => n,
                crate::ast::Type::Applied(n, _) => n,
                _ => continue,
            };
            let Some(resolved) = universe.types.get(type_name) else { continue };
            let Some(ref on_exit_fn) = resolved.on_exit else { continue };
            let Some(reg) = self.let_bindings.get(name) else { continue };
            // Emit: call void @on_exit_fn(i64 %reg)
            writeln!(out, "{}{} = call i64 @{}(i64 {})",
                indent,
                format!("%pcl{}", self.txn_counter),
                on_exit_fn,
                reg
            ).ok();
            self.txn_counter += 1;
        }
    }

    /// Check the target expression for an InsertAt strategy by looking up
    /// the variable's type in the TypeUniverse.
    pub(super) fn check_insert_strategy(&self, target: &crate::ast::Expr) -> Option<crate::type_universe::InsertStrategy> {
        let tu = self.type_universe.as_ref()?;
        let var_name = match target {
            crate::ast::Expr::OwnedRef(n) => n,
            crate::ast::Expr::Identifier(n) => n,
            _ => return None,
        };
        // Look up the variable's declared type
        let ty = self.let_original_types.get(var_name)?;
        let type_name = match ty {
            crate::ast::Type::Custom(n) => n,
            crate::ast::Type::Applied(n, _) => n,
            _ => return None,
        };
        tu.insert_strategy(type_name)
    }

    pub(super) fn emit_header(&self, out: &mut String) {
        writeln!(out, "; ModuleID = 'program.ll'").ok();
        writeln!(out, "source_filename = \"program.bv\"").ok();
        writeln!(out, "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"").ok();
        writeln!(out, "target triple = \"x86_64-unknown-linux-gnu\"").ok();
    }

    pub(super) fn emit_declares(&self, out: &mut String) {
        writeln!(out).ok();
        writeln!(out, "declare void @llvm.assume(i1) #1").ok();
        writeln!(out, "declare void @llvm.trap() noreturn").ok();
        // Intrinsic declares used by name#() instrinsic calls in emit_expr.
        // Previously these came from std/llvm.bv via as intrinsic, but the
        // name#() mechanism emits them directly without frgn_map entries.
        writeln!(out, "declare float @llvm.sqrt.f32(float) #1").ok();
        writeln!(out, "declare float @llvm.fabs.f32(float) #1").ok();
        writeln!(out, "declare float @llvm.ceil.f32(float) #1").ok();
        writeln!(out, "declare float @llvm.floor.f32(float) #1").ok();
        writeln!(out, "declare i64 @llvm.ctpop.i64(i64) #1").ok();
        writeln!(out, "declare i64 @llvm.ctlz.i64(i64, i1) #1").ok();
        writeln!(out, "declare i64 @llvm.cttz.i64(i64, i1) #1").ok();
        writeln!(out, "declare i64 @llvm.abs.i64(i64, i1) #1").ok();
        writeln!(out, "declare i64 @llvm.bitreverse.i64(i64) #1").ok();
        // Runtime support functions
        writeln!(out, "declare void @__barrier_release__()").ok();
        writeln!(out, "declare void @__barrier_wait__()").ok();
        writeln!(out, "declare void @__thread_pool_init__(i32, i8**)").ok();
        writeln!(out, "declare i64 @time(i64*) nounwind").ok();
        writeln!(out, "declare noalias i8* @malloc(i64) nounwind").ok();
        writeln!(out, "declare void @free(i8*) nounwind").ok();
        writeln!(out, "declare i64 @__read_file__(i64)").ok();
        writeln!(out, "declare i64 @__write_file__(i64, i64)").ok();
        writeln!(out, "declare i64 @__readln__()").ok();
        writeln!(out, "declare i64 @__sort_list__(i64)").ok();
        writeln!(out, "declare i64 @__reverse_list__(i64)").ok();
        writeln!(out, "declare i64 @__range__(i64)").ok();
        writeln!(out, "declare i64 @__trim_left__(ptr)").ok();
        writeln!(out, "declare i64 @__trim_right__(ptr)").ok();
        writeln!(out, "declare i64 @__to_lower__(ptr)").ok();
        writeln!(out, "declare i64 @__contains_at__(ptr, ptr, i64)").ok();
        writeln!(out, "declare i64 @__find_from__(ptr, ptr, i64)").ok();
        writeln!(out, "declare i64 @__splitn__(ptr, ptr, i64)").ok();
        writeln!(out, "declare i64 @__float_to_str(float)").ok();
        writeln!(out, "declare i64 @__to_str(i64)").ok();
        writeln!(out, "declare i64 @__stack_top__(i64)").ok();
        writeln!(out, "declare i64 @__queue_front__(i64)").ok();
        writeln!(out, "declare i64 @__hashmap_get__(i64, i64)").ok();
        writeln!(out, "declare i64 @__hashset_elements__(i64)").ok();
        writeln!(out, "declare void @__exit()").ok();
        writeln!(out, "declare i64 @__tty_raw_mode__(i64)").ok();
        writeln!(out, "declare i64 @__spawn_with_output__(i64)").ok();
        writeln!(out, "declare i64 @__readlink__(i64)").ok();
        writeln!(out, "declare i64 @__getcwd__()").ok();
        writeln!(out, "declare i64 @__readdir__(i64)").ok();
        writeln!(out, "declare i64 @__sigaction__(i64, i64)").ok();
        writeln!(out, "declare i64 @__sigprocmask__(i64, i64)").ok();
        writeln!(out, "declare i64 @__getaddrinfo__(i64, i64)").ok();
        writeln!(out, "declare i64 @__map_keys__(i64)").ok();
        writeln!(out, "declare i64 @__map_values__(i64)").ok();
        // D12–D18 + Extra Shim declares (2026-06-19)
        writeln!(out, "declare i64 @__errno__()").ok();
        writeln!(out, "declare i64 @__getrandom__(i64, i64, i64)").ok();
        writeln!(out, "declare i64 @__uname__()").ok();
        writeln!(out, "declare i64 @__hostname__()").ok();
        writeln!(out, "declare i64 @__strerror__(i64)").ok();
        writeln!(out, "declare i64 @__strsignal__(i64)").ok();
        writeln!(out, "declare i64 @__realpath__(i64)").ok();
        writeln!(out, "declare i64 @__backtrace__()").ok();
        writeln!(out, "declare i64 @__getpwuid__(i64)").ok();
        writeln!(out, "declare i64 @__getgrgid__(i64)").ok();
        writeln!(out, "declare i64 @__thread_create__(i64, i64)").ok();
        writeln!(out, "declare i64 @__thread_join__(i64)").ok();
        writeln!(out, "declare void @__thread_exit__(i64)").ok();
        writeln!(out, "declare i64 @__mutex_lock__(i64)").ok();
        writeln!(out, "declare i64 @__mutex_unlock__(i64)").ok();
        writeln!(out, "declare i64 @__condvar_wait__(i64, i64)").ok();
        writeln!(out, "declare i64 @__condvar_signal__(i64)").ok();
        writeln!(out, "declare i64 @__condvar_broadcast__(i64)").ok();
        writeln!(out, "declare i64 @__getrlimit__(i64)").ok();
        writeln!(out, "declare i64 @__setrlimit__(i64, i64)").ok();
        writeln!(out, "declare i64 @__mkstemp__(i64)").ok();
        writeln!(out, "declare i64 @__mkdtemp__(i64)").ok();
        writeln!(out, "declare i64 @__dlopen__(i64)").ok();
        writeln!(out, "declare i64 @__dlsym__(i64, i64)").ok();
        writeln!(out, "declare i64 @__dlclose__(i64)").ok();
        writeln!(out, "declare i64 @__ttyname__(i64)").ok();
        // Some externally-linked functions called by intrinsics
        writeln!(out, "declare i32 @ioctl(i32, i64, ptr)").ok();
    }

    pub(super) fn llvm_type(&self, ty: &Type) -> &str {
        match ty {
            Type::Int | Type::UInt => "i64",
            Type::Bool => "i8",
            Type::Float => "float",
            Type::Char => "i32",
            Type::String | Type::Data => "i8*",
            Type::Void => "void",
            _ => "i64",
        }
    }

    pub(super) fn native_float_or_box(&mut self, out: &mut String, indent: &str, val_reg: &str) -> String {
        if let Some(cached) = self.reg_float_cache.get(val_reg) {
            return cached.clone();
        }
        let tr = format!("%nftr{}", self.txn_counter); self.txn_counter += 1;
        let fl = format!("%nffl{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, val_reg).ok();
        writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
        fl
    }

    pub(super) fn ensure_float_reg(&mut self, out: &mut String, indent: &str, reg: &TypedRegister) -> String {
        // Check cache first — even float-typed registers may have their
        // native float counterpart cached (e.g. parameter marshaling boxes
        // float to i64 at function entry; the cache maps boxed→native).
        if let Some(cached) = self.reg_float_cache.get(&reg.name) {
            return cached.clone();
        }
        if reg.ty == Type::Float {
            // Native float, not in cache → already a native float register.
            return reg.name.clone();
        }
        self.native_float_or_box(out, indent, &reg.name)
    }

    /// Emit epoll-based initialization for built-in trigger sources.
    /// Creates an epoll fd, registers each built-in trigger's source fd,
    /// and stores the epfd in a synthetic state field.
    pub(super) fn emit_trg_init(&mut self, out: &mut String) {
        // Need at least one built-in trigger to emit setup
        let has_builtin = self.triggers.iter().any(|(_, trg)| matches!(
            &trg.address,
            crate::ast::LinkRef::Stdin | crate::ast::LinkRef::Timer(_) | crate::ast::LinkRef::Signal(_)
        ));
        if !has_builtin { return; }

        // Constants
        let epoin = 0x01u32; // EPOLLIN
        let epolet = 0x80000000u32; // EPOLLET
        let epoloneshot = 0x40000000u32; // EPOLLONESHOT
        let f_setfl = 4; // F_SETFL
        let o_nonblock = 0x800u32; // O_NONBLOCK
        let clo_monotonic = 1; // CLOCK_MONOTONIC
        let tfd_nonblock = 0x800; // TFD_NONBLOCK = O_NONBLOCK = 2048 on x86_64
        let sfd_nonblock = 0x800; // SFD_NONBLOCK = O_NONBLOCK = 2048 on x86_64
        let sig_block = 0; // SIG_BLOCK

        // epoll_create1(0)
        let epfd = format!("%epfd");
        writeln!(out, "  {} = call i32 @epoll_create1(i32 0)", epfd).ok();

        // Store epfd in epfd_field slot
        let sge = format!("%sge{}", self.txn_counter); self.txn_counter += 1;
        if let Some(epfd_idx) = self.field_index_map.get("__trg_epfd") {
            writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", sge, epfd_idx).ok();
            writeln!(out, "  store i32 {}, i32* {}, align 4", epfd, sge).ok();
        }

        // Per-trigger setup
        for (name, trg) in &self.triggers {
            let bit = self.dep_graph.bit_index.get(name).copied().unwrap_or(0);
            match &trg.address {
                crate::ast::LinkRef::Stdin => {
                    // fcntl(0, F_SETFL, O_NONBLOCK)
                    writeln!(out, "  %fcntl_{} = call i32 @fcntl(i32 0, i32 {}, i32 {})", name, f_setfl, o_nonblock).ok();
                    // epoll_event struct on stack: { events: EPOLLIN, data: { u64: bit } }
                    let ev_slot = format!("%ev_{}", name);
                    writeln!(out, "  {} = alloca i8, i64 16, align 8", ev_slot).ok();
                    let ev_events = format!("%eve_{}", name);
                    writeln!(out, "  {} = getelementptr i8, i8* {}, i64 0", ev_events, ev_slot).ok();
                    let ev_events_i32 = format!("%evei_{}", name);
                    writeln!(out, "  {} = bitcast i8* {} to i32*", ev_events_i32, ev_events).ok();
                    writeln!(out, "  store i32 {}, i32* {}, align 4", epoin, ev_events_i32).ok();
                    let ev_data = format!("%evd_{}", name);
                    writeln!(out, "  {} = getelementptr i8, i8* {}, i64 8", ev_data, ev_slot).ok();
                    let ev_data_u64 = format!("%evdu_{}", name);
                    writeln!(out, "  {} = bitcast i8* {} to i64*", ev_data_u64, ev_data).ok();
                    writeln!(out, "  store i64 {}, i64* {}, align 8", bit, ev_data_u64).ok();
                    // epoll_ctl(epfd, EPOLL_CTL_ADD, 0, &ev)
                    let ctl = format!("%ectl_{}", name);
                    writeln!(out, "  {} = call i32 @epoll_ctl(i32 {}, i32 1, i32 0, i8* {})", ctl, epfd, ev_slot).ok();
                }
                crate::ast::LinkRef::Timer(hz) => {
                    // timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK)
                    let tfd = format!("%tfd_{}", name);
                    writeln!(out, "  {} = call i32 @timerfd_create(i32 {}, i32 {})", tfd, clo_monotonic, tfd_nonblock).ok();
                    // timerfd_settime(tfd, 0, &its, null) — fires at Hz
                    let its_slot = format!("%its_{}", name);
                    writeln!(out, "  {} = alloca i8, i64 32, align 8", its_slot).ok();
                    // itimerspec.it_interval.tv_sec = 0
                    // itimerspec.it_interval.tv_nsec = 1_000_000_000 / hz
                    let interval_nsec = if *hz > 0 { 1_000_000_000u64 / *hz as u64 } else { 0 };
                    // itimerspec.it_value.tv_sec = 0
                    // itimerspec.it_value.tv_nsec = interval_nsec (fire immediately, then at interval)
                    let its_val_sec = format!("%its_vs_{}", name);
                    writeln!(out, "  {} = getelementptr i8, i8* {}, i64 0", its_val_sec, its_slot).ok();
                    let its_val_sec_i64 = format!("%itsvsi_{}", name);
                    writeln!(out, "  {} = bitcast i8* {} to i64*", its_val_sec_i64, its_val_sec).ok();
                    writeln!(out, "  store i64 0, i64* {}, align 8", its_val_sec_i64).ok();
                    let its_val_nsec = format!("%its_vn_{}", name);
                    writeln!(out, "  {} = getelementptr i8, i8* {}, i64 8", its_val_nsec, its_slot).ok();
                    let its_val_nsec_i64 = format!("%itsvni_{}", name);
                    writeln!(out, "  {} = bitcast i8* {} to i64*", its_val_nsec_i64, its_val_nsec).ok();
                    writeln!(out, "  store i64 {}, i64* {}, align 8", interval_nsec, its_val_nsec_i64).ok();
                    let its_int_sec = format!("%its_is_{}", name);
                    writeln!(out, "  {} = getelementptr i8, i8* {}, i64 16", its_int_sec, its_slot).ok();
                    let its_int_sec_i64 = format!("%itsisi_{}", name);
                    writeln!(out, "  {} = bitcast i8* {} to i64*", its_int_sec_i64, its_int_sec).ok();
                    writeln!(out, "  store i64 0, i64* {}, align 8", its_int_sec_i64).ok();
                    let its_int_nsec = format!("%its_in_{}", name);
                    writeln!(out, "  {} = getelementptr i8, i8* {}, i64 24", its_int_nsec, its_slot).ok();
                    let its_int_nsec_i64 = format!("%itsini_{}", name);
                    writeln!(out, "  {} = bitcast i8* {} to i64*", its_int_nsec_i64, its_int_nsec).ok();
                    writeln!(out, "  store i64 {}, i64* {}, align 8", interval_nsec, its_int_nsec_i64).ok();
                    writeln!(out, "  %tfd_settime_{} = call i32 @timerfd_settime(i32 {}, i32 0, i8* {}, i8* null)", name, tfd, its_slot).ok();
                    // epoll_ctl(epfd, EPOLL_CTL_ADD, tfd, &ev)
                    let ev_slot = format!("%ev_{}", name);
                    writeln!(out, "  {} = alloca i8, i64 16, align 8", ev_slot).ok();
                    let ev_events = format!("%eve_{}", name);
                    writeln!(out, "  {} = getelementptr i8, i8* {}, i64 0", ev_events, ev_slot).ok();
                    let ev_events_i32 = format!("%evei_{}", name);
                    writeln!(out, "  {} = bitcast i8* {} to i32*", ev_events_i32, ev_events).ok();
                    writeln!(out, "  store i32 {}, i32* {}, align 4", epoin, ev_events_i32).ok();
                    let ev_data = format!("%evd_{}", name);
                    writeln!(out, "  {} = getelementptr i8, i8* {}, i64 8", ev_data, ev_slot).ok();
                    let ev_data_u64 = format!("%evdu_{}", name);
                    writeln!(out, "  {} = bitcast i8* {} to i64*", ev_data_u64, ev_data).ok();
                    writeln!(out, "  store i64 {}, i64* {}, align 8", bit, ev_data_u64).ok();
                    writeln!(out, "  %ectl_{} = call i32 @epoll_ctl(i32 {}, i32 1, i32 {}, i8* {})", name, epfd, tfd, ev_slot).ok();
                }
                crate::ast::LinkRef::Signal(sig) => {
                    // sigemptyset(&mask)
                    let mask_slot = format!("%mask_{}", name);
                    writeln!(out, "  {} = alloca i8, i64 128, align 8", mask_slot).ok();
                    writeln!(out, "  %sigemptyset_{} = call i32 @sigemptyset(i8* {})", name, mask_slot).ok();
                    // sigaddset(&mask, SIG)
                    let sig_num = sig_number(sig);
                    writeln!(out, "  %sigadd_{} = call i32 @sigaddset(i8* {}, i32 {})", name, mask_slot, sig_num).ok();
                    // sigprocmask(SIG_BLOCK, &mask, null)
                    writeln!(out, "  %sigprocmask_{} = call i32 @sigprocmask(i32 {}, i8* {}, i8* null)", name, sig_block, mask_slot).ok();
                    // signalfd(-1, &mask, SFD_NONBLOCK)
                    let sfd = format!("%sfd_{}", name);
                    writeln!(out, "  {} = call i32 @signalfd(i32 -1, i8* {}, i32 {})", sfd, mask_slot, sfd_nonblock).ok();
                    // epoll_ctl(epfd, EPOLL_CTL_ADD, sfd, &ev)
                    let ev_slot = format!("%ev_{}", name);
                    writeln!(out, "  {} = alloca i8, i64 16, align 8", ev_slot).ok();
                    let ev_events = format!("%eve_{}", name);
                    writeln!(out, "  {} = getelementptr i8, i8* {}, i64 0", ev_events, ev_slot).ok();
                    let ev_events_i32 = format!("%evei_{}", name);
                    writeln!(out, "  {} = bitcast i8* {} to i32*", ev_events_i32, ev_events).ok();
                    writeln!(out, "  store i32 {}, i32* {}, align 4", epoin, ev_events_i32).ok();
                    let ev_data = format!("%evd_{}", name);
                    writeln!(out, "  {} = getelementptr i8, i8* {}, i64 8", ev_data, ev_slot).ok();
                    let ev_data_u64 = format!("%evdu_{}", name);
                    writeln!(out, "  {} = bitcast i8* {} to i64*", ev_data_u64, ev_data).ok();
                    writeln!(out, "  store i64 {}, i64* {}, align 8", bit, ev_data_u64).ok();
                    writeln!(out, "  %ectl_{} = call i32 @epoll_ctl(i32 {}, i32 1, i32 {}, i8* {})", name, epfd, sfd, ev_slot).ok();
                }
                _ => {} // Explicit(addr), Linked(sym) — handled by emit_trg_load in step()
            }
        }
    }

    /// Load an external trigger value (MMIO address or C global).
    /// Built-in triggers (@stdin#, @timer#, @signal#) are stored to state
    /// by the event loop — load from the state field.
    pub(super) fn emit_trg_load(&mut self, out: &mut String, indent: &str, dst: &str, address: &crate::ast::LinkRef, trg_ty: &Type) {
        match address {
            crate::ast::LinkRef::Explicit(addr) => {
                let store_ty = super::trg_llvm_storage_ty(trg_ty);
                let tr_counter = self.txn_counter;
                self.txn_counter += 1;
                let raw = format!("%tr{}", tr_counter);
                writeln!(out, "{}{} = load volatile {}, {}* inttoptr (i64 {} to {}*), align 1", indent, raw, store_ty, store_ty, addr, store_ty).ok();
                self.emit_trg_load_finish(out, indent, dst, raw, trg_ty);
            }
            crate::ast::LinkRef::Linked(sym) => {
                let store_ty = super::trg_llvm_storage_ty(trg_ty);
                let tr_counter = self.txn_counter;
                self.txn_counter += 1;
                let raw = format!("%tr{}", tr_counter);
                writeln!(out, "{}{} = load volatile {}, {}* @{}", indent, raw, store_ty, store_ty, sym).ok();
                self.emit_trg_load_finish(out, indent, dst, raw, trg_ty);
            }
            _ => {
                // Stdin, Timer, Signal — event loop stores values to state.
                // Emit add i64 0, 0 as zero default; callers with field access
                // patterns will have already loaded from state via Identifier -> field_index_map path.
                writeln!(out, "{}{} = add i64 0, 0 ; built-in trigger (loaded by event loop)", indent, dst).ok();
            }
        }
    }

    /// Finish loading a trigger value after the raw load: convert to native LLVM type.
    pub(super) fn emit_trg_load_finish(&self, out: &mut String, indent: &str, dst: &str, raw: String, trg_ty: &Type) {
        match trg_ty {
            Type::Bool => {
                writeln!(out, "{}{} = trunc i8 {} to i1", indent, dst, raw).ok();
            }
            Type::Int | Type::UInt => {
                writeln!(out, "{}{} = add i64 0, {}", indent, dst, raw).ok();
            }
            Type::Float => {
                writeln!(out, "{}{} = add float 0.0, {}", indent, dst, raw).ok();
            }
            Type::Char => {
                writeln!(out, "{}{} = zext i32 {} to i64", indent, dst, raw).ok();
            }
            Type::String | Type::Data => {
                writeln!(out, "{}{} = bitcast i8* {} to i8*", indent, dst, raw).ok();
            }
            _ => {
                writeln!(out, "{}{} = add i64 0, {}", indent, dst, raw).ok();
            }
        }
    }

    pub(super) fn align_of(&self, ty: &str) -> u32 {
        match ty {
            "i64" => 8,
            "float" => 4,
            "i8" => 1,
            "i32" => 4,
            _ => 8,
        }
    }

    pub(super) fn declare_state_type(&mut self, out: &mut String) {
        if self.field_types.is_empty() {
            writeln!(out, "%State = type {{ i64 }}").ok();
            return;
        }
        write!(out, "%State = type {{ ").ok();
        for (i, f) in self.field_types.iter().enumerate() {
            if i > 0 { write!(out, ", ").ok(); }
            write!(out, "{}", f).ok();
        }
        writeln!(out, " }}").ok();
    }

    pub(super) fn emit_init_state(&mut self, out: &mut String) {
        writeln!(out, "define void @init_state(%State* noalias nocapture align 8 %state) local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        let mut reg = 0u32;
        let mut fields: Vec<(String, usize, String)> = self.field_index_map.iter()
            .map(|(name, &idx)| (name.clone(), idx, self.field_types[idx].clone()))
            .collect();
        fields.sort_by_key(|&(_, idx, _)| idx);
        for (name, idx, ty) in fields {
            let p = format!("%ip{}", reg); reg += 1;
            writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", p, idx).ok();
            let init_clone = self.field_initializers.get(&name).and_then(|e| e.clone());
            match init_clone {
                Some(Expr::Integer(n)) => {
                    writeln!(out, "  store i64 {}, i64* {}, align {}", n, p, self.align_of("i64")).ok();
                }
                Some(Expr::Float(f)) => {
                    let h = float_to_llvm_hex(f);
                    let bits_reg = format!("%ip{}b", reg - 1);
                    writeln!(out, "  {} = bitcast i32 {} to float", bits_reg, h).ok();
                    writeln!(out, "  store float {}, float* {}, align {}", bits_reg, p, self.align_of("float")).ok();
                }
                Some(Expr::Neg(ref inner)) => {
                    match inner.as_ref() {
                        Expr::Float(f) => {
                            let h = float_to_llvm_hex(-*f);
                            let bits_reg = format!("%ip{}b", reg - 1);
                            writeln!(out, "  {} = bitcast i32 {} to float", bits_reg, h).ok();
                            writeln!(out, "  store float {}, float* {}, align {}", bits_reg, p, self.align_of("float")).ok();
                        }
                        Expr::Literal(lit) => {
                            if let crate::features::literal::LiteralExpr::Float(f) = lit.as_ref() {
                                let h = float_to_llvm_hex(-*f);
                                let bits_reg = format!("%ip{}b", reg - 1);
                                writeln!(out, "  {} = bitcast i32 {} to float", bits_reg, h).ok();
                                writeln!(out, "  store float {}, float* {}, align {}", bits_reg, p, self.align_of("float")).ok();
                            } else {
                                writeln!(out, "  store i64 0, i64* {}, align {}", p, self.align_of("i64")).ok();
                            }
                        }
                        Expr::Integer(n) => {
                            writeln!(out, "  store i64 -{}, i64* {}, align {}", n, p, self.align_of("i64")).ok();
                        }
                        _ => {
                            writeln!(out, "  store i64 0, i64* {}, align {}", p, self.align_of("i64")).ok();
                        }
                    }
                }
                Some(Expr::Bool(b)) => {
                    let v = if b { "1" } else { "0" };
                    writeln!(out, "  store i8 {}, i8* {}, align {}", v, p, self.align_of("i8")).ok();
                }
                Some(Expr::Literal(lit)) if matches!(lit.as_ref(), crate::features::literal::LiteralExpr::String(_)) => {
                    let s = match lit.as_ref() {
                        crate::features::literal::LiteralExpr::String(s) => s,
                        _ => unreachable!(),
                    };
                    let si = self.string_constants.iter().position(|x| *x == *s).unwrap_or(0);
                    let g = format!("@str.{}", si);
                    let str_p = format!("%ip{}s", reg); reg += 1;
                    writeln!(out, "  {} = bitcast <{{ i64, i64, [{} x i8] }}>* {} to i8*", str_p, s.len() + 1, g).ok();
                    writeln!(out, "  store i8* {}, i8** {}, align {}", str_p, p, self.align_of("i8*")).ok();
                }
                Some(Expr::String(s)) => {
                    let si = self.string_constants.iter().position(|x| *x == *s).unwrap_or(0);
                    let g = format!("@str.{}", si);
                    let str_p = format!("%ip{}s", reg); reg += 1;
                    writeln!(out, "  {} = bitcast <{{ i64, i64, [{} x i8] }}>* {} to i8*", str_p, s.len() + 1, g).ok();
                    writeln!(out, "  store i8* {}, i8** {}, align {}", str_p, p, self.align_of("i8*")).ok();
                }
                Some(Expr::Char(c)) => {
                    let v = c as i32;
                    writeln!(out, "  store i32 {}, i32* {}, align {}", v, p, self.align_of("i32")).ok();
                }
                // 2026-06-20: Handle LiteralExpr::Float directly, matching Expr::Float arm above.
                // Same rationale as emit_inline_init_stores arm at line ~560.
                Some(Expr::Literal(lit)) if matches!(lit.as_ref(), crate::features::literal::LiteralExpr::Float(_)) => {
                    if let crate::features::literal::LiteralExpr::Float(f) = lit.as_ref() {
                        let h = crate::backend::llvm::float_to_llvm_hex(*f);
                        let bits_reg = format!("%ip{}b", reg - 1);
                        writeln!(out, "  {} = bitcast i32 {} to float", bits_reg, h).ok();
                        writeln!(out, "  store float {}, float* {}, align {}", bits_reg, p, self.align_of("float")).ok();
                    }
                }
                Some(expr) => {
                    let val_reg = self.emit_expr(out, &expr, "  ");
                    let boxed = self.adapt_to_i64(out, "  ", &val_reg);
                    match ty.as_str() {
                        "i8" => {
                            let t = format!("%ip{}t", reg); reg += 1;
                            writeln!(out, "  {} = trunc i64 {} to i8", t, boxed).ok();
                            writeln!(out, "  store i8 {}, i8* {}, align {}", t, p, self.align_of("i8")).ok();
                        }
                        "i32" => {
                            let t = format!("%ip{}t", reg); reg += 1;
                            writeln!(out, "  {} = trunc i64 {} to i32", t, boxed).ok();
                            writeln!(out, "  store i32 {}, i32* {}, align {}", t, p, self.align_of("i32")).ok();
                        }
                        "float" => {
                            let fl = self.native_float_or_box(out, "  ", &val_reg.to_string());
                            writeln!(out, "  store float {}, float* {}, align {}", fl, p, self.align_of("float")).ok();
                        }
                        "i8*" => {
                            let t = format!("%ip{}t", reg); reg += 1;
                            writeln!(out, "  {} = inttoptr i64 {} to i8*", t, boxed).ok();
                            writeln!(out, "  store i8* {}, i8** {}, align {}", t, p, self.align_of("i8*")).ok();
                        }
                        _ => {
                            writeln!(out, "  store {} {}, {}* {}, align {}", ty, boxed, ty, p, self.align_of(&ty)).ok();
                        }
                    }
                }
                None => {
                    let default = if ty == "i8*" { "null".to_string() } else { "0".to_string() };
                    writeln!(out, "  store {} {}, {}* {}, align {}", ty, default, ty, p, self.align_of(&ty)).ok();
                }
            }
        }
        let mmio_inits: Vec<(u64, Expr)> = {
            let mut v = Vec::new();
            for (name, &addr) in &self.mmio_fields {
                if let Some(Some(expr)) = self.mmio_initializers.get(name).cloned() {
                    v.push((addr, expr.clone()));
                }
            }
            v
        };
        for (addr, expr) in mmio_inits {
            let p = format!("%mio{}", reg); reg += 1;
            writeln!(out, "  {} = inttoptr i64 {} to i64*", p, addr).ok();
            let val_reg = self.emit_expr(out, &expr, "  ");
            writeln!(out, "  store volatile i64 {}, i64* {}, align 1", val_reg, p).ok();
        }
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
    }

    /// Emit field initialization stores inline in the current function (no function call).
    /// Same logic as emit_init_state but stores are emitted directly, not wrapped in a
    /// separate @init_state function. This prevents the %state alloca from escaping to
    /// init_state, enabling LLVM's SROA to decompose the %State struct.
    pub(super) fn emit_inline_init_stores(&mut self, out: &mut String, state_ptr: &str) {
        let indent = if state_ptr == "%state" { "  " } else { "" };
        let mut fields: Vec<(String, usize, String)> = self.field_index_map.iter()
            .map(|(name, &idx)| (name.clone(), idx, self.field_types[idx].clone()))
            .collect();
        fields.sort_by_key(|&(_, idx, _)| idx);
        for (name, idx, _ty) in &fields {
            let p = format!("%ip_{}", idx);
            writeln!(out, "{}{} = getelementptr inbounds %State, %State* {}, i32 0, i32 {}", indent, p, state_ptr, idx).ok();
            let init_clone = self.field_initializers.get(name).and_then(|e| e.clone());
            let ty = self.field_types[*idx].clone();
            match init_clone {
                Some(Expr::Integer(n)) => {
                    writeln!(out, "{}store i64 {}, i64* {}, align {}", indent, n, p, self.align_of("i64")).ok();
                }
                Some(Expr::Float(f)) => {
                    let h = float_to_llvm_hex(f);
                    let bits_reg = format!("%ip_{}b", idx);
                    writeln!(out, "{}{} = bitcast i32 {} to float", indent, bits_reg, h).ok();
                    writeln!(out, "{}store float {}, float* {}, align {}", indent, bits_reg, p, self.align_of("float")).ok();
                }
                Some(Expr::Neg(ref inner)) => {
                    match inner.as_ref() {
                        Expr::Float(f) => {
                            let h = float_to_llvm_hex(-*f);
                            let bits_reg = format!("%ip_{}b", idx);
                            writeln!(out, "{}{} = bitcast i32 {} to float", indent, bits_reg, h).ok();
                            writeln!(out, "{}store float {}, float* {}, align {}", indent, bits_reg, p, self.align_of("float")).ok();
                        }
                        Expr::Literal(lit) => {
                            if let crate::features::literal::LiteralExpr::Float(f) = lit.as_ref() {
                                let h = float_to_llvm_hex(-*f);
                                let bits_reg = format!("%ip_{}b", idx);
                                writeln!(out, "{}{} = bitcast i32 {} to float", indent, bits_reg, h).ok();
                                writeln!(out, "{}store float {}, float* {}, align {}", indent, bits_reg, p, self.align_of("float")).ok();
                            } else {
                                writeln!(out, "{}store i64 0, i64* {}, align {}", indent, p, self.align_of("i64")).ok();
                            }
                        }
                        Expr::Integer(n) => {
                            writeln!(out, "{}store i64 -{}, i64* {}, align {}", indent, n, p, self.align_of("i64")).ok();
                        }
                        _ => {
                            writeln!(out, "{}store i64 0, i64* {}, align {}", indent, p, self.align_of("i64")).ok();
                        }
                    }
                }
                Some(Expr::Bool(b)) => {
                    let v = if b { "1" } else { "0" };
                    writeln!(out, "{}store i8 {}, i8* {}, align {}", indent, v, p, self.align_of("i8")).ok();
                }
                Some(Expr::Literal(lit)) if matches!(lit.as_ref(), crate::features::literal::LiteralExpr::String(_)) => {
                    // 2026-06-17: Store string constant pointer for LiteralExpr::String
                    let s = match lit.as_ref() {
                        crate::features::literal::LiteralExpr::String(s) => s,
                        _ => unreachable!(),
                    };
                    let si = self.string_constants.iter().position(|x| *x == *s).unwrap_or(0);
                    let g = format!("@str.{}", si);
                    let str_p = format!("%ip_{}s", idx);
                    writeln!(out, "{}{} = bitcast <{{ i64, i64, [{} x i8] }}>* {} to i8*", indent, str_p, s.len() + 1, g).ok();
                    let tag_p = format!("%ip_{}t", idx);
                    writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, tag_p, str_p).ok();
                    let tag_o = format!("%ip_{}o", idx);
                    writeln!(out, "{}{} = or i64 {}, 1", indent, tag_o, tag_p).ok();
                    let tag_b = format!("%ip_{}b", idx);
                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, tag_b, tag_o).ok();
                    writeln!(out, "{}store i8* {}, i8** {}, align {}", indent, tag_b, p, self.align_of("i8*")).ok();
                }
                Some(Expr::String(s)) => {
                    // 2026-06-17: Store actual string constant pointer, not null.
                    // The string is stored as a bitcast of @str.N to i8*, matching
                    // what Expr::String emits in emit_expr.rs:32.
                    let si = self.string_constants.iter().position(|x| *x == *s).unwrap_or(0);
                    let g = format!("@str.{}", si);
                    let str_p = format!("%ip_{}s", idx);
                    writeln!(out, "{}{} = bitcast <{{ i64, i64, [{} x i8] }}>* {} to i8*", indent, str_p, s.len() + 1, g).ok();
                    // Tag with bit 0 = 1 to mark as static (not heap-allocated)
                    let tag_p = format!("%ip_{}t", idx);
                    writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, tag_p, str_p).ok();
                    let tag_o = format!("%ip_{}o", idx);
                    writeln!(out, "{}{} = or i64 {}, 1", indent, tag_o, tag_p).ok();
                    let tag_b = format!("%ip_{}b", idx);
                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, tag_b, tag_o).ok();
                    writeln!(out, "{}store i8* {}, i8** {}, align {}", indent, tag_b, p, self.align_of("i8*")).ok();
                }
                Some(Expr::Char(c)) => {
                    let v = c as i32;
                    writeln!(out, "{}store i32 {}, i32* {}, align {}", indent, v, p, self.align_of("i32")).ok();
                }
                // 2026-06-20: Handle LiteralExpr::Float directly, matching Expr::Float arm above.
                // Without this, the catch-all boxes the float to i64 and immediately unboxes it
                // back, producing dead IR. LLVM DCE would clean them, but they may cause verifier
                // errors if they cross adapt_to_i64 before DCE runs.
                Some(Expr::Literal(lit)) if matches!(lit.as_ref(), crate::features::literal::LiteralExpr::Float(_)) => {
                    if let crate::features::literal::LiteralExpr::Float(f) = lit.as_ref() {
                        let h = crate::backend::llvm::float_to_llvm_hex(*f);
                        let bits_reg = format!("%ip_{}b", idx);
                        writeln!(out, "{}{} = bitcast i32 {} to float", indent, bits_reg, h).ok();
                        writeln!(out, "{}store float {}, float* {}, align {}", indent, bits_reg, p, self.align_of("float")).ok();
                    }
                }
                Some(expr) => {
                    let val_reg = self.emit_expr(out, &expr, indent);
                    let boxed = self.adapt_to_i64(out, indent, &val_reg);
                    match ty.as_str() {
                        "i8" => {
                            let t = format!("%ip_{}t", idx);
                            writeln!(out, "{}{} = trunc i64 {} to i8", indent, t, boxed).ok();
                            writeln!(out, "{}store i8 {}, i8* {}, align {}", indent, t, p, self.align_of("i8")).ok();
                        }
                        "i32" => {
                            let t = format!("%ip_{}t", idx);
                            writeln!(out, "{}{} = trunc i64 {} to i32", indent, t, boxed).ok();
                            writeln!(out, "{}store i32 {}, i32* {}, align {}", indent, t, p, self.align_of("i32")).ok();
                        }
                        "float" => {
                            let fl = self.native_float_or_box(out, indent, &val_reg.to_string());
                            writeln!(out, "{}store float {}, float* {}, align {}", indent, fl, p, self.align_of("float")).ok();
                        }
                        "i8*" => {
                            let t = format!("%ip_{}t", idx);
                            writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, t, boxed).ok();
                            writeln!(out, "{}store i8* {}, i8** {}, align {}", indent, t, p, self.align_of("i8*")).ok();
                        }
                        _ => {
                            writeln!(out, "{}store {} {}, {}* {}, align {}", indent, ty, boxed, ty, p, self.align_of(&ty)).ok();
                        }
                    }
                }
                None => {
                    let default = if ty == "i8*" { "null".to_string() } else { "0".to_string() };
                    writeln!(out, "{}store {} {}, {}* {}, align {}", indent, ty, default, ty, p, self.align_of(&ty)).ok();
                }
            }
        }
        // Initialize cache slots for LazyCached fields: cache_value = 0, valid_flag = 0
        for (_field_name, (cache_idx, valid_idx)) in &self.cache_slots {
            let cp = format!("%icp_{}", cache_idx);
            writeln!(out, "{}{} = getelementptr inbounds %State, %State* {}, i32 0, i32 {}", indent, cp, state_ptr, cache_idx).ok();
            writeln!(out, "{}store i64 0, i64* {}, align {}", indent, cp, self.align_of("i64")).ok();
            let vp = format!("%ivp_{}", valid_idx);
            writeln!(out, "{}{} = getelementptr inbounds %State, %State* {}, i32 0, i32 {}", indent, vp, state_ptr, valid_idx).ok();
            writeln!(out, "{}store i8 0, i8* {}, align {}", indent, vp, self.align_of("i8")).ok();
        }
    }

    pub(super) fn emit_definition(&mut self, out: &mut String, d: &crate::ast::Definition) {
        self.pending_cleanup.clear();
        self.let_bindings.clear(); self.let_binding_types.clear(); self.let_original_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
        // 2026-06-17: Use correct LLVM return type (float for Float, otherwise i64)
        let is_float_fn = d.outputs.iter().any(|t| matches!(t, Type::Float));
        let ll_ret_ty = if is_float_fn { "float" } else { "i64" };
        self.fn_ret_ty = ll_ret_ty.to_string();
        self.returns_i64 = !is_float_fn;
        // Rename user `main` to `brief_main` to avoid collision with
        // the runtime entry point `define i32 @main()` in loop_engine.rs.
        let ll_name: &str = if d.name == "main" { "brief_main" } else { &d.name };
        write!(out, "define {} @{}(", ll_ret_ty, ll_name).ok();
        write!(out, "%State* noalias nocapture align 8 %state").ok();
        for (i, (n, t)) in d.parameters.iter().enumerate() {
            write!(out, ", {} %arg{}", self.llvm_type(t), i).ok();
        }
        writeln!(out, ") local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        for (i, (n, t)) in d.parameters.iter().enumerate() {
            let raw = format!("%arg{}", i);
            let conv = format!("%ac{}", i);
            let reg: String;
            if matches!(t, Type::Bool | Type::Char | Type::String | Type::Data | Type::Float) {
                match t {
                    Type::Bool => { writeln!(out, "  {} = zext i8 {} to i64", conv, raw).ok(); }
                    Type::Char => { writeln!(out, "  {} = zext i32 {} to i64", conv, raw).ok(); }
                    Type::String | Type::Data => { writeln!(out, "  {} = ptrtoint i8* {} to i64", conv, raw).ok(); }
                    Type::Float => {
                        let m = format!("%ai{}", i);
                        writeln!(out, "  {} = bitcast float {} to i32", m, raw).ok();
                        writeln!(out, "  {} = zext i32 {} to i64", conv, m).ok();
                        self.reg_float_cache.insert(conv.clone(), raw.to_string());
                    }
                    _ => {}
                }
                reg = conv;
            } else {
                reg = raw;
            }
            self.let_bindings.insert(n.clone(), reg.clone());
            // Boxed params (Bool/Char/String/Data) are stored as i64,
            // so mark them as Type::Int so downstream doesn't treat them
            // as native i1/i32/i8*. Float stays Type::Float (handled specially).
                if matches!(t, Type::Bool | Type::Char | Type::String | Type::Data) {
                    self.let_binding_types.insert(n.clone(), Type::Int);
                    // 2026-06-17: Save original type for String/Data params so
                    // is_string_chain can detect string variables by original type.
                    self.let_original_types.insert(n.clone(), t.clone());
                } else {
                    self.let_binding_types.insert(n.clone(), t.clone());
                }
        }
        self.txn_counter = 0;
        self.terminated = false;
        for s in &d.body {
            if self.terminated { break; }
            self.emit_stmt(out, s, "  ");
        }
        // Foreign destructor cleanup: emit OnExit calls before returning
        self.emit_on_exit_cleanup(out, "  ");
        if !self.terminated {
            if is_float_fn {
                writeln!(out, "  ret float 0.0").ok();
            } else {
                writeln!(out, "  ret i64 0").ok();
            }
        }
        writeln!(out, "}}").ok();
        // Phase 4.5: Emit dso_local export wrapper if #export modifier present
        if let Some(export_name) = Self::get_export_name(&d.modifiers) {
            writeln!(out, "define dso_local {} @{}(" , ll_ret_ty, export_name).ok();
            write!(out, "%State* %state").ok();
            for (i, (n, t)) in d.parameters.iter().enumerate() {
                write!(out, ", {} %arg{}", self.llvm_type(t), i).ok();
            }
            writeln!(out, ") local_unnamed_addr #0 {{").ok();
            write!(out, "  %res = call {} @{}(", ll_ret_ty, ll_name).ok();
            write!(out, "%State* %state").ok();
            for (i, (n, t)) in d.parameters.iter().enumerate() {
                write!(out, ", {} %arg{}", self.llvm_type(t), i).ok();
            }
            writeln!(out, ") local_unnamed_addr #0").ok();
            writeln!(out, "  ret {} %res", ll_ret_ty).ok();
            writeln!(out, "}}").ok();
        }
    }
    // 2026-06-13: Added %State* %state param — definitions can access global state.
    // Was missing the state pointer, causing invalid LLVM IR (SSA value out of scope).

    pub(super) fn emit_transaction(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str, range_meta: &mut Vec<String>) {
        let has_output = txn.output_type.is_some() || !txn.outputs.is_empty();
        if !txn.is_reactive && (!txn.parameters.is_empty() || has_output) {
            self.emit_callable_txn(out, txn, name);
            return;
        }
        self.pending_cleanup.clear();
        self.range_bounds = Self::extract_ranges(&txn.contract.pre_condition);
        self.field_to_meta_idx.clear();
        for (f, &(lo, hi)) in &self.range_bounds {
            if hi < i64::MAX {
                let mi = range_meta.len();
                let dlo = if lo > i64::MIN { lo } else { i64::MIN };
                range_meta.push(format!("!{} = !{{ i64 {}, i64 {} }}", mi, dlo, hi));
                self.field_to_meta_idx.insert(f.clone(), mi);
            }
        }
        // Resolve #inline / #?inline directives from transaction modifiers.
        // #inline forces alwaysinline regardless of cycles.
        // #?inline emits inlinehint (safe with cycles).
        // If neither, fall back to cycle-based alwaysinline.
        let alwaysinline = {
            let dirs = super::directive::resolve_directives(
                &txn.modifiers,
                super::directive::DirectiveCtx::Transaction,
            );
            let inline_attr = dirs.iter().find_map(|e| {
                if let super::directive::DirectiveEffect::FunctionAttribute(a) = e {
                    Some(a.as_str())
                } else {
                    None
                }
            });
            // Emit remarks for speculative #?inline directives.
            if txn.modifiers.iter().any(|m| m.name == "inline" && m.speculative) {
                let remark = match inline_attr {
                    Some("inlinehint") => {
                        super::directive::OptimizationRemark::applied("inline",
                            format!("inlinehint applied to txn '{}'", name))
                    }
                    _ => {
                        super::directive::OptimizationRemark::skipped("inline",
                            "inlinehint not applicable for this context".to_string())
                    }
                };
                self.push_remark(remark);
            }
            match inline_attr {
                Some("alwaysinline") => " alwaysinline",
                Some("inlinehint") => " inlinehint",
                _ => {
                    if !self.has_cycles { " alwaysinline" } else { "" }
                }
            }
        };
        let txn_attr = self.slp_attr(name, "#0");

        let assume_action: Option<&str> = txn.modifiers.iter()
            .find(|m| m.name == "assume_shape")
            .and_then(|m| m.value.as_ref())
            .and_then(|v| {
                let parts: Vec<&str> = v.splitn(2, ", ").collect();
                if parts.len() == 2 {
                    let action = parts[1].trim();
                    if action == "run" || action == "exit" { Some(action) } else { Some("escape") }
                } else {
                    Some("escape")
                }
            });

        if let Some(action) = assume_action {
            writeln!(out, "define void @{}(%State* noalias nocapture align 8 %state) local_unnamed_addr {}{} {{", name, txn_attr, alwaysinline).ok();
            writeln!(out, "  entry:").ok();
            writeln!(out, "  br i1 true, label %body, label %rollback").ok();
            writeln!(out, "  body:").ok();
            self.txn_counter = 0;
            self.let_bindings.clear(); self.let_binding_types.clear(); self.let_original_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
            self.terminated = false;
            self.returns_i64 = false;
            if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
                self.emit_precondition_check(out, &txn.contract.pre_condition, "  ");
            }
            let (reordered, has_cycle) = super::reorder::reorder_body_statements(&txn.body);
            if has_cycle {
                self.warnings.push(format!(
                    "Warning: dependency cycle detected in transaction '{}' — ILP reordering is suboptimal",
                    name
                ));
            }
            for s in &reordered {
                if self.terminated { break; }
                self.emit_stmt(out, s, "  ");
            }
            if !self.terminated { writeln!(out, "  ret void").ok(); }
            writeln!(out, "  rollback:").ok();
            match action {
                "exit" => {
                    writeln!(out, "    call void @__exit(i64 1)").ok();
                    writeln!(out, "    unreachable").ok();
                }
                "run" => {
                    writeln!(out, "    br label %body").ok();
                }
                _ => {
                    writeln!(out, "    ret void").ok();
                }
            }
            writeln!(out, "}}").ok();
        } else {
            writeln!(out, "define void @{}(%State* noalias nocapture align 8 %state) local_unnamed_addr {}{} {{", name, txn_attr, alwaysinline).ok();
            writeln!(out, "  entry:").ok();
            self.txn_counter = 0;
            self.let_bindings.clear(); self.let_binding_types.clear(); self.let_original_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
            self.terminated = false;
            self.returns_i64 = false;
            if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
                self.emit_precondition_check(out, &txn.contract.pre_condition, "  ");
            }
            let (reordered, has_cycle) = super::reorder::reorder_body_statements(&txn.body);
            if has_cycle {
                self.warnings.push(format!(
                    "Warning: dependency cycle detected in transaction '{}' — ILP reordering is suboptimal",
                    name
                ));
            }
            for s in &reordered {
                if self.terminated { break; }
                self.emit_stmt(out, s, "  ");
            }
            if !self.terminated { writeln!(out, "  ret void").ok(); }
            writeln!(out, "}}").ok();
        }

        // Collect GPU kernel for this transaction if it has #gpu / #!gpu / #?gpu.
        if self.gpu_offload || txn.modifiers.iter().any(|m| m.name == "gpu") {
            let is_speculative = txn.modifiers.iter()
                .any(|m| m.name == "gpu" && m.speculative);
            self.collect_gpu_kernel(name, &txn.body, is_speculative);
        }
    }

    pub(super) fn emit_callable_txn(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        self.pending_cleanup.clear();
        self.let_bindings.clear();
        self.let_binding_types.clear();
        self.let_original_types.clear(); self.reg_float_cache.clear();
        self.reg_type_cache.clear();
        self.param_slots.clear();

        let has_return = if let Some(ref ot) = txn.output_type {
            match ot {
                crate::ast::OutputType::Single(ty) => !matches!(ty, Type::Void),
                crate::ast::OutputType::Tuple(ts) => !ts.is_empty(),
                _ => false,
            }
        } else {
            !txn.outputs.is_empty() && !matches!(txn.outputs.first(), Some(Type::Void))
        };
        let ret_llvm = if has_return { "i64" } else { "void" };

        // Resolve #inline / #?inline directives for callable txns.
        let inline_attr = {
            let dirs = super::directive::resolve_directives(
                &txn.modifiers,
                super::directive::DirectiveCtx::CallableTxn,
            );
            dirs.iter().find_map(|e| {
                if let super::directive::DirectiveEffect::FunctionAttribute(a) = e {
                    Some(format!(" {}", a))
                } else {
                    None
                }
            })
        };
        let inline_str = inline_attr.as_deref().unwrap_or("");

        write!(out, "define {} @{}(", ret_llvm, name).ok();
        write!(out, "%State* noalias nocapture align 8 %state").ok();
        for (i, (n, t)) in txn.parameters.iter().enumerate() {
            write!(out, ", {} %arg{}", self.llvm_type(t), i).ok();
        }
        writeln!(out, ") local_unnamed_addr #0{} {{", inline_str).ok();
        writeln!(out, "  entry:").ok();

        writeln!(out, "  %result = alloca i64, align 8").ok();
        writeln!(out, "  store i64 0, i64* %result, align 8").ok();

        for (i, (n, t)) in txn.parameters.iter().enumerate() {
            let raw = format!("%arg{}", i);
            let conv: String;
            if matches!(t, Type::Bool | Type::Char | Type::String | Type::Data | Type::Float) {
                let ac = format!("%ac{}", i);
                match t {
                    Type::Bool => { writeln!(out, "  {} = zext i8 {} to i64", ac, raw).ok(); }
                    Type::Char => { writeln!(out, "  {} = zext i32 {} to i64", ac, raw).ok(); }
                    Type::String | Type::Data => { writeln!(out, "  {} = ptrtoint i8* {} to i64", ac, raw).ok(); }
                    Type::Float => {
                        let m = format!("%ai{}", i);
                        writeln!(out, "  {} = bitcast float {} to i32", m, raw).ok();
                        writeln!(out, "  {} = zext i32 {} to i64", ac, m).ok();
                    }
                    _ => {}
                }
                conv = ac;
            } else {
                conv = raw;
            }
            let slot = format!("%p{}_s", i);
            writeln!(out, "  {} = alloca i64, align 8", slot).ok();
            writeln!(out, "  store i64 {}, i64* {}, align 8", conv, slot).ok();
            self.param_slots.insert(n.clone(), slot);
        }

        writeln!(out, "  br label %loop").ok();
        writeln!(out, "loop:").ok();

        for (i, (n, t)) in txn.parameters.iter().enumerate() {
            let slot = format!("%p{}_s", i);
            let loaded = format!("%p{}_l{}", i, self.txn_counter);
            self.txn_counter += 1;
            writeln!(out, "  {} = load i64, i64* {}, align 8", loaded, slot).ok();
            self.let_bindings.insert(n.clone(), loaded);
            // loaded is i64 (boxed value from param slot). Store Type::Int
            // for boxed types so downstream doesn't treat them as native.
            if matches!(t, Type::Bool | Type::Char | Type::String | Type::Data | Type::Float) {
                self.let_binding_types.insert(n.clone(), Type::Int);
            } else {
                self.let_binding_types.insert(n.clone(), t.clone());
            }
        }

        self.callable_txn_result = Some("%result".to_string());
        self.callable_txn_post_label = Some("post".to_string());
        self.in_callable_txn = true;
        self.txn_counter = 0;
        self.terminated = false;
        self.returns_i64 = has_return;

        if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
            let cond = self.emit_expr(out, &txn.contract.pre_condition, "  ");
            let i1 = format!("%pc{}", self.txn_counter); self.txn_counter += 1;
            if cond.ty == Type::Bool {
                writeln!(out, "  {} = and i1 {}, true", i1, cond).ok();
            } else {
                writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
            }
            writeln!(out, "  br i1 {}, label %body, label %done", i1).ok();
        } else {
            writeln!(out, "  br label %body").ok();
        }

        writeln!(out, "body:").ok();

        for s in &txn.body {
            if self.terminated { break; }
            self.emit_stmt(out, s, "  ");
        }

        // Foreign destructor cleanup: emit OnExit calls before loop exit
        self.emit_on_exit_cleanup(out, "  ");

        if !self.terminated {
            writeln!(out, "  br label %post").ok();
        }
        writeln!(out, "post:").ok();
        writeln!(out, "  br label %loop").ok();

        writeln!(out, "done:").ok();
        if has_return {
            let ret = format!("%ret{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "  {} = load i64, i64* %result, align 8", ret).ok();
            writeln!(out, "  ret i64 {}", ret).ok();
        } else {
            writeln!(out, "  ret void").ok();
        }
        writeln!(out, "}}").ok();

        self.callable_txn_result = None;
        self.callable_txn_post_label = None;
        self.in_callable_txn = false;
        self.param_slots.clear();
    }

    pub(super) fn emit_precondition_check(&mut self, out: &mut String, pre: &Expr, indent: &str) {
        let cond = self.emit_expr(out, pre, indent);
        let i1 = format!("%pi{}", self.txn_counter); self.txn_counter += 1;
        if cond.ty == Type::Bool {
            // cond is already i1 (native bool)
            writeln!(out, "{}{} = and i1 {}, true", indent, i1, cond).ok();
        } else {
            writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, i1, cond).ok();
        }
        let panic_l = format!("pp{}", self.txn_counter); self.txn_counter += 1;
        let safe_l = format!("ps{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, i1, safe_l, panic_l).ok();
        writeln!(out, "{}{}:", indent, panic_l).ok();
        writeln!(out, "{}  unreachable", indent).ok();
        writeln!(out, "{}{}:", indent, safe_l).ok();
        // Replace @llvm.assume with !range metadata for simple patterns.
        // Pattern: [x < N] on a state field known to ssa_old_int_regs.
        // We emit a re-load of x with !range { 0, N } — the extra load is
        // GVN-eliminated if the bound is already provable by LLVM.
        match pre {
            Expr::Lt(lhs, rhs) if matches!(rhs.as_ref(), Expr::Integer(_)) => {
                if let Expr::Identifier(name) = lhs.as_ref() {
                    if let Some(&idx) = self.field_index_map.get(name) {
                        let bound = if let Expr::Integer(b) = rhs.as_ref() { *b } else { 0 };
                        let gep = format!("%prg{}", self.txn_counter); self.txn_counter += 1;
                        let ty = &self.field_types[idx];
                        writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, gep, idx).ok();
                        let rl = format!("%prl{}", self.txn_counter); self.txn_counter += 1;
                        let tn = crate::backend::llvm::tbaa_node(ty);
                        writeln!(out, "{}{} = load {}, {}* {}, align {}, !tbaa !{}, !range !{{{}, {}}}",
                            indent, rl, ty, ty, gep, self.align_of(ty), tn, 0i64, bound).ok();
                    } else {
                        writeln!(out, "{}call void @llvm.assume(i1 {})", indent, i1).ok();
                    }
                } else {
                    writeln!(out, "{}call void @llvm.assume(i1 {})", indent, i1).ok();
                }
            }
            _ => {
                writeln!(out, "{}call void @llvm.assume(i1 {})", indent, i1).ok();
            }
        }
    }

    pub(super) fn emit_pre_function(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        if matches!(txn.contract.pre_condition, Expr::Bool(true)) { return; }
        writeln!(out, "define internal i1 @pre_{}(%State* noalias nocapture align 8 %state) #0 {{", name).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0;
        self.let_bindings.clear(); self.let_binding_types.clear(); self.let_original_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
        let cond = self.emit_expr(out, &txn.contract.pre_condition, "  ");
        if cond.ty == Type::Bool {
            writeln!(out, "  ret i1 {}", cond).ok();
        } else {
            let i1 = format!("%ri{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
            writeln!(out, "  ret i1 {}", i1).ok();
        }
        writeln!(out, "}}").ok();

        // Collect GPU kernel for callable txns with #gpu directives.
        if self.gpu_offload || txn.modifiers.iter().any(|m| m.name == "gpu") {
            let is_speculative = txn.modifiers.iter()
                .any(|m| m.name == "gpu" && m.speculative);
            self.collect_gpu_kernel(name, &txn.body, is_speculative);
        }
    }

    pub(super) fn emit_async_body(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        let async_name = format!("async_body_{}", name);
        let async_attr = self.slp_attr(&async_name, "#0");
        writeln!(out, "define void @{}(%State* noalias nocapture align 8 %state) local_unnamed_addr {} {{", async_name, async_attr).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0;
        self.let_bindings.clear(); self.let_binding_types.clear(); self.let_original_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
        let cond = self.emit_expr(out, &txn.contract.pre_condition, "  ");
        let i1 = if cond.ty == Type::Bool {
            cond.name.clone()
        } else {
            let i1 = format!("%ri{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
            i1
        };
        let txn_fire_l = format!("txn_fire_{}", self.txn_counter + 1);
        writeln!(out, "  br i1 {}, label %{}, label %{}_done", i1, txn_fire_l, async_name).ok();
        writeln!(out, "{}:", txn_fire_l).ok();
        self.terminated = false;
        self.returns_i64 = false;
        for s in &txn.body {
            if self.terminated { break; }
            self.emit_stmt(out, s, "  ");
        }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "{}_done:", async_name).ok();
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
    }

    pub(super) fn emit_fused(&mut self, out: &mut String, a: &crate::ast::Transaction, b: &crate::ast::Transaction, name: &str) {
        let body_a: Vec<Statement> = a.body.iter()
            .filter(|s| !matches!(s, Statement::Term { .. } | Statement::TermBang { .. } | Statement::Escape(_)))
            .cloned().collect();
        let combined: Vec<Statement> = body_a.into_iter().chain(b.body.iter().cloned()).collect();
        let fused_attr = self.slp_attr(name, "#0");
        writeln!(out, "define void @{}(%State* noalias nocapture align 8 %state) local_unnamed_addr {} {{", name, fused_attr).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0; self.let_bindings.clear(); self.let_binding_types.clear(); self.let_original_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear(); self.terminated = false; self.returns_i64 = false;
        for s in &combined {
            if self.terminated { break; }
            self.emit_stmt(out, s, "  ");
        }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "}}").ok();
    }

    pub(super) fn emit_shape_guarded_body(&mut self, out: &mut String, body: &[Statement], name: &str, action: &str) {
        let fused_attr = self.slp_attr(name, "#0");
        writeln!(out, "define void @{}(%State* noalias nocapture align 8 %state) local_unnamed_addr {} {{", name, fused_attr).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  br i1 true, label %body, label %rollback").ok();
        writeln!(out, "  body:").ok();
        self.txn_counter = 0; self.let_bindings.clear(); self.let_binding_types.clear(); self.let_original_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();         self.terminated = false; self.returns_i64 = false;
        for s in body {
            if self.terminated { break; }
            self.emit_stmt(out, s, "  ");
        }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "  rollback:").ok();
        match action {
            "exit" => {
                writeln!(out, "    call void @__exit(i64 1)").ok();
                writeln!(out, "    unreachable").ok();
            }
            "run" => {
                writeln!(out, "    br label %body").ok();
            }
            _ => {
                writeln!(out, "    ret void").ok();
            }
        }
        writeln!(out, "}}").ok();
    }

    pub(super) fn emit_fused_composed(&mut self, out: &mut String, body: &[Statement], name: &str) {
        let fused_attr = self.slp_attr(name, "#0");
        writeln!(out, "define void @{}(%State* noalias nocapture align 8 %state) local_unnamed_addr {} {{", name, fused_attr).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0; self.let_bindings.clear(); self.let_binding_types.clear(); self.let_original_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear(); self.terminated = false; self.returns_i64 = false;
        for s in body {
            if self.terminated { break; }
            self.emit_stmt(out, s, "  ");
        }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "}}").ok();
    }

    /// Emit a user-defined `inop#` / `inop!#` intrinsic as a private LLVM function.
    /// The body is pasted verbatim from the inop# declaration, with `term`
    /// replaced by `ret` (or `br %post_label` in callable txn context).
    pub(super) fn emit_inop(&mut self, out: &mut String, inop: &crate::ast::InopDeclaration) {
        let is_float_fn = inop.outputs.iter().any(|t| {
            let resolved = self.resolve_bild_type(t);
            matches!(resolved, Type::Float)
        });

        // Detect multi-output: term %q, %r — has comma-separated registers
        let is_multi_output = inop.llvm_body.iter().any(|line| {
            let trimmed = line.trim();
            (trimmed.starts_with("term ") || trimmed.starts_with("term!"))
                && trimmed.contains(',')
        });

        let ll_ret_ty = if is_multi_output {
            let count = inop.outputs.len().max(2);
            let tys: Vec<&str> = (0..count).map(|_| if is_float_fn { "float" } else { "i64" }).collect();
            format!("{{{}}}", tys.join(", "))
        } else if is_float_fn {
            "float".to_string()
        } else {
            "i64".to_string()
        };
        self.fn_ret_ty = ll_ret_ty.clone();
        self.returns_i64 = !is_float_fn && !is_multi_output;

        write!(out, "define {} @{}(", ll_ret_ty, inop.name).ok();
        if inop.has_state_access {
            write!(out, "%State* noalias nocapture align 8 %state").ok();
        }
        for (n, t) in &inop.params {
            let resolved = self.resolve_bild_type(t);
            let native_ty = self.llvm_type(&resolved);
            write!(out, ", {} %{}", native_ty, n).ok();
            self.let_bindings.insert(n.clone(), format!("%{}", n));
            self.let_binding_types.insert(n.clone(), resolved.clone());
            self.let_original_types.insert(n.clone(), t.clone());
        }
        writeln!(out, ") local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0;
        self.terminated = false;

        for line in &inop.llvm_body {
            let trimmed = line.trim();
            if trimmed.starts_with("term!") {
                let after = trimmed.strip_prefix("term!").unwrap_or("").trim();
                if !after.is_empty() {
                    writeln!(out, "  store i64 {}, ptr %state, align 8", after).ok();
                }
                writeln!(out, "  br label %done").ok();
                self.terminated = true;
            } else if trimmed == "term" || trimmed.starts_with("term ") {
                let after = trimmed.strip_prefix("term").map(|s| s.trim()).unwrap_or("");
                if !after.is_empty() {
                    if is_multi_output {
                        // Multi-output: term %q, %r → insertvalue chain + ret struct
                        let regs: Vec<&str> = after.split(',').map(|s| s.trim()).collect();
                        for (i, reg) in regs.iter().enumerate() {
                            let base_ty = if is_float_fn { "float" } else { "i64" };
                            if i == 0 {
                                writeln!(out, "  %mv{} = insertvalue {} undef, {} {}, 0", self.txn_counter, ll_ret_ty, base_ty, reg).ok();
                            } else {
                                writeln!(out, "  %mv{} = insertvalue {} %mv{}, {} {}, {}", self.txn_counter, ll_ret_ty, self.txn_counter - 1, base_ty, reg, i).ok();
                            }
                            self.txn_counter += 1;
                        }
                        writeln!(out, "  ret {} %mv{}", ll_ret_ty, self.txn_counter - 1).ok();
                    } else if is_float_fn { writeln!(out, "  ret float {}", after).ok(); }
                    else { writeln!(out, "  ret i64 {}", after).ok(); }
                } else {
                    if is_float_fn { writeln!(out, "  ret float 0.0").ok(); }
                    else { writeln!(out, "  ret i64 0").ok(); }
                }
                self.terminated = true;
            } else {
                writeln!(out, "  {}", line).ok();
            }
        }
        if !self.terminated {
            if is_float_fn { writeln!(out, "  ret float 0.0").ok(); }
            else { writeln!(out, "  ret i64 0").ok(); }
        }
        writeln!(out, "}}").ok();
    }
}

/// Map a Brief signal name (e.g. "SIGWINCH", "SIGINT") to its POSIX number.
fn sig_number(name: &str) -> i32 {
    match name {
        "SIGHUP" => 1,
        "SIGINT" => 2,
        "SIGQUIT" => 3,
        "SIGILL" => 4,
        "SIGTRAP" => 5,
        "SIGABRT" => 6,
        "SIGBUS" => 7,
        "SIGFPE" => 8,
        "SIGKILL" => 9,
        "SIGUSR1" => 10,
        "SIGSEGV" => 11,
        "SIGUSR2" => 12,
        "SIGPIPE" => 13,
        "SIGALRM" => 14,
        "SIGTERM" => 15,
        "SIGSTKFLT" => 16,
        "SIGCHLD" => 17,
        "SIGCONT" => 18,
        "SIGSTOP" => 19,
        "SIGTSTP" => 20,
        "SIGTTIN" => 21,
        "SIGTTOU" => 22,
        "SIGURG" => 23,
        "SIGXCPU" => 24,
        "SIGXFSZ" => 25,
        "SIGVTALRM" => 26,
        "SIGPROF" => 27,
        "SIGWINCH" => 28,
        "SIGIO" => 29,
        "SIGPWR" => 30,
        "SIGSYS" => 31,
        _ => 0,
    }
}
