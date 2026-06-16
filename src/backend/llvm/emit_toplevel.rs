use crate::ast::{Expr, Statement, TopLevel, Type};
use crate::backend::llvm::{float_to_llvm_hex, LlvmBackend, TypedRegister};
use std::fmt::Write;

impl LlvmBackend {
    pub(super) fn emit_header(&self, out: &mut String) {
        writeln!(out, "; ModuleID = 'program.ll'").ok();
        writeln!(out, "source_filename = \"program.bv\"").ok();
        writeln!(out, "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"").ok();
        writeln!(out, "target triple = \"x86_64-unknown-linux-gnu\"").ok();
    }

    pub(super) fn emit_declares(&self, out: &mut String) {
        writeln!(out).ok();
        writeln!(out, "declare void @llvm.assume(i1) #1").ok();
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
        writeln!(out, "declare void @brief_barrier_release()").ok();
        writeln!(out, "declare void @brief_barrier_wait()").ok();
        writeln!(out, "declare void @brief_thread_pool_init(i32, i8**)").ok();
        writeln!(out, "declare i64 @time(i64*) nounwind").ok();
        writeln!(out, "declare noalias i8* @malloc(i64) nounwind").ok();
        writeln!(out, "declare ptr @brief_read_file(ptr)").ok();
        writeln!(out, "declare void @__exit()").ok();
        // Phase A: Terminal intrinsics (intrinsics.md D4)
        writeln!(out, "declare i64 @brief_tty_raw_mode(i64)").ok();
        writeln!(out, "declare i64 @brief_tty_size()").ok();
        writeln!(out, "declare i64 @brief_tty_read_key()").ok();
        writeln!(out, "declare i64 @brief_ioctl(i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_isatty(i64)").ok();
        // Phase A: Process intrinsics (intrinsics.md D5)
        writeln!(out, "declare i64 @brief_spawn_with_output(i64)").ok();
        writeln!(out, "declare i64 @brief_spawn(i64)").ok();
        // Phase B: Raw File I/O intrinsics (intrinsics.md D2)
        writeln!(out, "declare i64 @brief_open(i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_close(i64)").ok();
        writeln!(out, "declare i64 @brief_read(i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_write(i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_lseek(i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_pread(i64, i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_pwrite(i64, i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_stat(i64)").ok();
        writeln!(out, "declare i64 @brief_fstat(i64)").ok();
        writeln!(out, "declare i64 @brief_truncate(i64, i64)").ok();
        writeln!(out, "declare i64 @brief_ftruncate(i64, i64)").ok();
        writeln!(out, "declare i64 @brief_fsync(i64)").ok();
        writeln!(out, "declare i64 @brief_dup(i64)").ok();
        writeln!(out, "declare i64 @brief_dup2(i64, i64)").ok();
        writeln!(out, "declare i64 @brief_fcntl(i64, i64, i64)").ok();
        // Phase C: Filesystem intrinsics (intrinsics.md D3)
        writeln!(out, "declare i64 @brief_mkdir(i64, i64)").ok();
        writeln!(out, "declare i64 @brief_rmdir(i64)").ok();
        writeln!(out, "declare i64 @brief_unlink(i64)").ok();
        writeln!(out, "declare i64 @brief_rename(i64, i64)").ok();
        writeln!(out, "declare i64 @brief_symlink(i64, i64)").ok();
        writeln!(out, "declare i64 @brief_readlink(i64)").ok();
        writeln!(out, "declare i64 @brief_link(i64, i64)").ok();
        writeln!(out, "declare i64 @brief_getcwd()").ok();
        writeln!(out, "declare i64 @brief_chdir(i64)").ok();
        writeln!(out, "declare i64 @brief_readdir(i64)").ok();
        writeln!(out, "declare i64 @brief_chmod(i64, i64)").ok();
        writeln!(out, "declare i64 @brief_chown(i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_umask(i64)").ok();
        writeln!(out, "declare i64 @brief_access(i64, i64)").ok();
        // Phase D: Memory intrinsics (intrinsics.md D1)
        writeln!(out, "declare i64 @brief_mmap(i64, i64, i64, i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_munmap(i64, i64)").ok();
        writeln!(out, "declare i64 @brief_mprotect(i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_brk(i64)").ok();
        writeln!(out, "declare i64 @brief_mlock(i64, i64)").ok();
        // Phase D: Sync intrinsics — futex (Shim); atomic ops are Native (no declare needed)
        writeln!(out, "declare i64 @brief_futex(i64, i64, i64, i64, i64, i64)").ok();
        // Phase E: IPC intrinsics (intrinsics.md D11)
        writeln!(out, "declare i64 @brief_pipe(i64)").ok();
        writeln!(out, "declare i64 @brief_shm_open(i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_shm_unlink(i64)").ok();
        writeln!(out, "declare i64 @brief_sem_open(i64, i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_sem_wait(i64)").ok();
        writeln!(out, "declare i64 @brief_sem_post(i64)").ok();
        // Phase F: Signals intrinsics (intrinsics.md D8)
        writeln!(out, "declare i64 @brief_sigaction(i64, i64)").ok();
        writeln!(out, "declare i64 @brief_sigprocmask(i64, i64)").ok();
        writeln!(out, "declare i64 @brief_kill(i64, i64)").ok();
        writeln!(out, "declare i64 @brief_signalfd(i64)").ok();
        writeln!(out, "declare i64 @brief_timerfd_create(i64)").ok();
        // Phase G: Networking intrinsics (intrinsics.md D10)
        writeln!(out, "declare i64 @brief_socket(i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_bind(i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_listen(i64, i64)").ok();
        writeln!(out, "declare i64 @brief_accept(i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_connect(i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_send(i64, i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_recv(i64, i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_sendto(i64, i64, i64, i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_recvfrom(i64, i64, i64, i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_setsockopt(i64, i64, i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_getsockopt(i64, i64, i64, i64, i64)").ok();
        writeln!(out, "declare i64 @brief_shutdown(i64, i64)").ok();
        writeln!(out, "declare i64 @brief_getaddrinfo(i64, i64)").ok();
        // Phase H: Everything Else (intrinsics.md D6, D7)
        writeln!(out, "declare i64 @brief_getenv(i64)").ok();
        writeln!(out, "declare i64 @brief_setenv(i64, i64)").ok();
        writeln!(out, "declare i64 @brief_unsetenv(i64)").ok();
        writeln!(out, "declare i64 @brief_getpid()").ok();
        writeln!(out, "declare i64 @brief_getppid()").ok();
        writeln!(out, "declare i64 @brief_clock_gettime(i64)").ok();
        writeln!(out, "declare i64 @brief_nanosleep(i64)").ok();
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
        // native float counterpart cached (e.g. FFI return value boxed as i64).
        if let Some(cached) = self.reg_float_cache.get(&reg.name) {
            return cached.clone();
        }
        if reg.ty == Type::Float {
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
        let tfd_nonblock = 0x400; // TFD_NONBLOCK
        let sfd_nonblock = 0x400; // SFD_NONBLOCK
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
                writeln!(out, "{}{} = add i32 0, {}", indent, dst, raw).ok();
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
        writeln!(out, "define void @init_state(%State* noalias nocapture %state) local_unnamed_addr #0 {{").ok();
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
                Some(Expr::String(_)) => {
                    writeln!(out, "  store i8* null, i8** {}, align {}", p, self.align_of("i8*")).ok();
                }
                Some(Expr::Char(c)) => {
                    let v = c as i32;
                    writeln!(out, "  store i32 {}, i32* {}, align {}", v, p, self.align_of("i32")).ok();
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
                Some(Expr::String(_)) => {
                    writeln!(out, "{}store i8* null, i8** {}, align {}", indent, p, self.align_of("i8*")).ok();
                }
                Some(Expr::Char(c)) => {
                    let v = c as i32;
                    writeln!(out, "{}store i32 {}, i32* {}, align {}", indent, v, p, self.align_of("i32")).ok();
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
    }

    pub(super) fn emit_definition(&mut self, out: &mut String, d: &crate::ast::Definition) {
        self.pending_cleanup.clear();
        self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
        write!(out, "define i64 @{}(", d.name).ok();
        write!(out, "%State* noalias nocapture %state").ok();
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
            } else {
                self.let_binding_types.insert(n.clone(), t.clone());
            }
        }
        self.txn_counter = 0;
        self.terminated = false;
        self.returns_i64 = true;
        for s in &d.body {
            if self.terminated { break; }
            self.emit_stmt(out, s, "  ");
        }
        if !self.terminated { writeln!(out, "  ret i64 0").ok(); }
        writeln!(out, "}}").ok();
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
        let alwaysinline = if !self.has_cycles { " alwaysinline" } else { "" };
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
            writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {}{} {{", name, txn_attr, alwaysinline).ok();
            writeln!(out, "  entry:").ok();
            writeln!(out, "  br i1 true, label %body, label %rollback").ok();
            writeln!(out, "  body:").ok();
            self.txn_counter = 0;
            self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
            self.terminated = false;
            self.returns_i64 = false;
            if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
                self.emit_precondition_check(out, &txn.contract.pre_condition, "  ");
            }
            let reordered = super::reorder::reorder_body_statements(&txn.body);
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
            writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {}{} {{", name, txn_attr, alwaysinline).ok();
            writeln!(out, "  entry:").ok();
            self.txn_counter = 0;
            self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
            self.terminated = false;
            self.returns_i64 = false;
            if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
                self.emit_precondition_check(out, &txn.contract.pre_condition, "  ");
            }
            let reordered = super::reorder::reorder_body_statements(&txn.body);
            for s in &reordered {
                if self.terminated { break; }
                self.emit_stmt(out, s, "  ");
            }
            if !self.terminated { writeln!(out, "  ret void").ok(); }
            writeln!(out, "}}").ok();
        }
    }

    pub(super) fn emit_callable_txn(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        self.pending_cleanup.clear();
        self.let_bindings.clear();
        self.let_binding_types.clear();
        self.reg_float_cache.clear();
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

        write!(out, "define {} @{}(", ret_llvm, name).ok();
        write!(out, "%State* noalias nocapture %state").ok();
        for (i, (n, t)) in txn.parameters.iter().enumerate() {
            write!(out, ", {} %arg{}", self.llvm_type(t), i).ok();
        }
        writeln!(out, ") local_unnamed_addr #0 {{").ok();
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
        writeln!(out, "{}call void @llvm.assume(i1 {})", indent, i1).ok();
    }

    pub(super) fn emit_pre_function(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        if matches!(txn.contract.pre_condition, Expr::Bool(true)) { return; }
        writeln!(out, "define internal i1 @pre_{}(%State* noalias nocapture %state) #0 {{", name).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0;
        self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
        let cond = self.emit_expr(out, &txn.contract.pre_condition, "  ");
        if cond.ty == Type::Bool {
            writeln!(out, "  ret i1 {}", cond).ok();
        } else {
            let i1 = format!("%ri{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
            writeln!(out, "  ret i1 {}", i1).ok();
        }
        writeln!(out, "}}").ok();
    }

    pub(super) fn emit_async_body(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        let async_name = format!("async_body_{}", name);
        let async_attr = self.slp_attr(&async_name, "#0");
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {} {{", async_name, async_attr).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0;
        self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
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
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {} {{", name, fused_attr).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0; self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear(); self.terminated = false; self.returns_i64 = false;
        for s in &combined {
            if self.terminated { break; }
            self.emit_stmt(out, s, "  ");
        }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "}}").ok();
    }

    pub(super) fn emit_shape_guarded_body(&mut self, out: &mut String, body: &[Statement], name: &str, action: &str) {
        let fused_attr = self.slp_attr(name, "#0");
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {} {{", name, fused_attr).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  br i1 true, label %body, label %rollback").ok();
        writeln!(out, "  body:").ok();
        self.txn_counter = 0; self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();         self.terminated = false; self.returns_i64 = false;
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
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {} {{", name, fused_attr).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0; self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear(); self.terminated = false; self.returns_i64 = false;
        for s in body {
            if self.terminated { break; }
            self.emit_stmt(out, s, "  ");
        }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
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
