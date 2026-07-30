; ModuleID = 'program.ll'
source_filename = "program.bv"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

declare void @llvm.assume(i1) #1
declare void @llvm.trap() noreturn
declare float @llvm.sqrt.f32(float) #1
declare float @llvm.fabs.f32(float) #1
declare float @llvm.ceil.f32(float) #1
declare float @llvm.floor.f32(float) #1
declare double @llvm.sqrt.f64(double) #1
declare double @llvm.fabs.f64(double) #1
declare double @llvm.ceil.f64(double) #1
declare double @llvm.floor.f64(double) #1
declare i64 @llvm.ctpop.i64(i64) #1
declare i64 @llvm.ctlz.i64(i64, i1) #1
declare i64 @llvm.cttz.i64(i64, i1) #1
declare i64 @llvm.abs.i64(i64, i1) #1
declare i64 @llvm.bitreverse.i64(i64) #1
declare void @__barrier_release__()
declare void @__barrier_wait__()
declare void @__thread_pool_init__(i32, ptr)
declare void @__set_async_state__(ptr)
declare i64 @time(ptr) nounwind
declare i64 @atol(ptr) nounwind
declare ptr @getenv(ptr) nounwind
declare noalias ptr @malloc(i64) nounwind
declare void @free(ptr) nounwind
declare ptr @realloc(ptr, i64) nounwind
declare i64 @ShellCmd(i64)
declare i64 @brief_syscall(i64, i64, i64, i64, i64, i64, i64)
declare i64 @brief_sysconf(i64)
declare ptr @dlopen(ptr, i32) nounwind
declare ptr @dlsym(ptr, ptr) nounwind
declare i32 @dlclose(ptr) nounwind
declare i64 @brief_backtrace()
declare void @__print_str(i128) #6
declare i64 @__print_char(i64) #6
declare i64 @__getenv_int(i128) #6
declare i128 @__getenv_brief(i128) #6
declare i64 @__print_int(i64) #6
declare i64 @__print_float(float) #6
declare i8* @__chr_to_str(i32) #1
declare i64 @__int_to_str__(i64) #1
declare i64 @__str_bytes__(i64) #1
declare i64 @__str_to_int(i8*) #1
declare i64 @brief_open(i64, i64, i64) #1
declare i64 @brief_close(i64) #1
declare i64 @brief_read(i64, i64, i64) #1
declare i64 @brief_write(i64, i64, i64) #1
declare i64 @brief_lseek(i64, i64, i64) #1
declare i64 @brief_pread(i64, i64, i64, i64) #1
declare i64 @brief_pwrite(i64, i64, i64, i64) #1
declare i64 @brief_stat(i64, i64) #1
declare i64 @brief_fstat(i64) #1
declare i64 @brief_truncate(i64, i64) #1
declare i64 @brief_ftruncate(i64, i64) #1
declare i64 @brief_fsync(i64) #1
declare i64 @brief_dup(i64) #1
declare i64 @brief_dup2(i64, i64) #1
declare i64 @brief_fcntl(i64, i64, i64) #1
declare i64 @brief_socket(i64, i64, i64) #1
declare i64 @brief_bind(i64, i64, i64) #1
declare i64 @brief_listen(i64, i64) #1
declare i64 @brief_accept(i64, i64, i64) #1
declare i64 @brief_connect(i64, i64, i64) #1
declare i64 @brief_send(i64, i64, i64, i64) #1
declare i64 @brief_recv(i64, i64, i64, i64) #1
declare i64 @brief_sendto(i64, i64, i64, i64, i64, i64) #1
declare i64 @brief_recvfrom(i64, i64, i64, i64, i64, i64) #1
declare i64 @brief_setsockopt(i64, i64, i64, i64, i64) #1
declare i64 @brief_getsockopt(i64, i64, i64, i64, i64) #1
declare i64 @brief_shutdown(i64, i64) #1
declare i64 @brief_mkdir(i64, i64) #1
declare i64 @brief_rmdir(i64) #1
declare i64 @brief_unlink(i64) #1
declare i64 @brief_rename(i64, i64) #1
declare i64 @brief_symlink(i64, i64) #1
declare i64 @brief_link(i64, i64) #1
declare i64 @brief_chdir(i64) #1
declare i64 @brief_chmod(i64, i64) #1
declare i64 @brief_chown(i64, i64, i64) #1
declare i64 @brief_umask(i64) #1
declare i64 @brief_access(i64, i64) #1
declare i64 @brief_mmap(i64, i64, i64, i64, i64, i64) #1
declare i64 @brief_munmap(i64, i64) #1
declare i64 @brief_mprotect(i64, i64, i64) #1
declare i64 @brief_brk(i64) #1
declare i64 @brief_mlock(i64, i64) #1
declare i64 @brief_pipe(i64) #1
declare i64 @brief_shm_open(i64, i64, i64) #1
declare i64 @brief_shm_unlink(i64) #1
declare i64 @brief_sem_open(i64, i64, i64, i64) #1
declare i64 @brief_sem_wait(i64) #1
declare i64 @brief_sem_post(i64) #1
declare i64 @brief_getpid() #1
declare i64 @brief_getppid() #1
declare i64 @brief_clock_gettime(i64, i64) #1
declare i64 @brief_nanosleep(i64, i64) #1
declare i64 @brief_getenv(i64, i64, i64) #1
declare i64 @brief_setenv(i64, i64, i64) #1
declare i64 @brief_unsetenv(i64) #1
declare i64 @brief_futex(i64, i64, i64, i64, i64, i64) #1
declare i64 @__ioctl__(i64, i64, i64) #1
declare i64 @__isatty__(i64) #1
declare i64 @__print(i64) #1
declare i64 @brief_getuid() #1
declare i64 @brief_geteuid() #1
declare i64 @brief_getgid() #1
declare i64 @brief_getegid() #1
declare i64 @brief_sched_yield() #1
declare i64 @brief_getpriority(i64, i64) #1
declare i64 @brief_setpriority(i64, i64, i64) #1
declare i64 @brief_getrlimit(i64) #1
declare i64 @brief_setrlimit(i64, i64) #1
declare i64 @brief_pagesize() #1
declare i64 @brief_cpu_count() #1
declare i64 @brief_ttyname(i64) #1
declare i64 @brief_ring_push(i64, i64) #1
declare i64 @brief_ring_pop(i64) #1
declare i64 @__tty_read_key__(i64) #1
declare i64 @__tty_size__() #1
declare i64 @cpu_count() #1
declare i64 @pagesize() #1
@STR_READFILE_ERR = private unnamed_addr constant [15 x i8] c"file not found\00"
@stdout = external dso_local global ptr
declare void @__wait_for_trigger__() #1
%StateChunk0 = type { i64 }
%State = type { i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant <{ i64, [1 x i8] }> <{ i64 0, [1 x i8] c"\00" }>, align 8

@ll_empty_list = private unnamed_addr constant { i64, i64 } { i64 0, i64 0 }

define ptr @get_env(ptr noundef noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
    %t2 = inttoptr i64 %ac0 to ptr
   %t0 = call ptr @__getenv_brief(ptr %t2)
  ret ptr %t0
}

define i64 @get_env_int(ptr noundef noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
    %t2 = inttoptr i64 %ac0 to ptr
   %t0 = call i64 @__getenv_int(ptr %t2)
  ret i64 %t0
}

define i64 @tame(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, ptr %arg2, i64 %arg3) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %t3 = add i64 0, 0
  %t1 = add nsw i64 %ac0, %t3
  %t4 = inttoptr i64 %t1 to ptr
  %t0 = load i64, ptr %t4, align 8
  %t7 = add i64 0, 4294967295
  %t5 = and i64 %t0, %t7
  %t10 = add i64 0, 1380532556
  %t11 = icmp ne i64 %t5, %t10
  %t8 = zext i1 %t11 to i8
  %t13 = trunc i8 %t8 to i1
  br i1 %t13, label %guard.then12, label %guard.end12
  guard.then12:
  %t14 = add i64 0, 101
  ret i64 %t14
  br label %guard.end12
  guard.end12:
  %t20 = add i64 0, 4
  %t18 = add nsw i64 %ac0, %t20
  %t21 = inttoptr i64 %t18 to ptr
  %t17 = load i64, ptr %t21, align 8
  %t26 = add i64 0, 5
  %t24 = add nsw i64 %ac0, %t26
  %t27 = inttoptr i64 %t24 to ptr
  %t23 = load i64, ptr %t27, align 8
  %t32 = add i64 0, 6
  %t30 = add nsw i64 %ac0, %t32
  %t33 = inttoptr i64 %t30 to ptr
  %t29 = load i64, ptr %t33, align 8
  %t38 = add i64 0, 7
  %t36 = add nsw i64 %ac0, %t38
  %t39 = inttoptr i64 %t36 to ptr
  %t35 = load i64, ptr %t39, align 8
  %t43 = add i64 0, 20
  %t41 = sdiv i64 %t23, %t43
  %t44 = add nsw i64 %ac0, %t29
  %t47 = add nsw i64 %ac0, %t17
  %t51 = add nsw i64 %t29, %t35
  %t55 = icmp sgt i64 %t51, %arg1
  %t50 = zext i1 %t55 to i8
  %t57 = trunc i8 %t50 to i1
  br i1 %t57, label %guard.then56, label %guard.end56
  guard.then56:
  %t58 = add i64 0, 103
  ret i64 %t58
  br label %guard.end56
  guard.end56:
  %t62 = add nsw i64 %t17, %t23
  %t66 = icmp sgt i64 %t62, %arg1
  %t61 = zext i1 %t66 to i8
  %t68 = trunc i8 %t61 to i1
  br i1 %t68, label %guard.then67, label %guard.end67
  guard.then67:
  %t69 = add i64 0, 104
  ret i64 %t69
  br label %guard.end67
  guard.end67:
  %t74 = add i64 0, 0
  %t75 = icmp eq i64 %t41, %t74
  %t72 = zext i1 %t75 to i8
  %t77 = trunc i8 %t72 to i1
  br i1 %t77, label %guard.then76, label %guard.end76
  guard.then76:
  %t78 = add i64 0, 105
  ret i64 %t78
  br label %guard.end76
  guard.end76:
  %t86 = inttoptr i64 %t47 to ptr
  %t87 = inttoptr i64 %t44 to ptr
  %t81 = call i64 @compute_buffer_sizes(ptr %t86, i64 %t41, ptr %t87, i64 %t35)
  %t90 = add i64 0, 1024
  %t91 = icmp sle i64 %t81, %t90
  %t88 = zext i1 %t91 to i8
  %t93 = trunc i8 %t88 to i1
  br i1 %t93, label %guard.then92, label %guard.end92
  guard.then92:
  %t96 = add i64 0, 0
  %t97 = icmp sge i64 %t81, %t96
  %t94 = zext i1 %t97 to i8
  %t99 = trunc i8 %t94 to i1
  br i1 %t99, label %gate.pass98, label %loop
  gate.pass98:
  br label %guard.end92
  guard.end92:
  %t103 = load i64, ptr @locals_slots
  %t104 = add i64 0, 4096
  %t105 = icmp sle i64 %t103, %t104
  %t102 = zext i1 %t105 to i8
  %t107 = trunc i8 %t102 to i1
  br i1 %t107, label %guard.then106, label %guard.end106
  guard.then106:
  %t109 = load i64, ptr @locals_slots
  %t110 = add i64 0, 0
  %t111 = icmp sge i64 %t109, %t110
  %t108 = zext i1 %t111 to i8
  %t113 = trunc i8 %t108 to i1
  br i1 %t113, label %gate.pass112, label %loop
  gate.pass112:
  br label %guard.end106
  guard.end106:
  %t117 = load i64, ptr @frames_max
  %t118 = add i64 0, 256
  %t119 = icmp sle i64 %t117, %t118
  %t116 = zext i1 %t119 to i8
  %t121 = trunc i8 %t116 to i1
  br i1 %t121, label %guard.then120, label %guard.end120
  guard.then120:
  %t123 = load i64, ptr @frames_max
  %t124 = add i64 0, 0
  %t125 = icmp sgt i64 %t123, %t124
  %t122 = zext i1 %t125 to i8
  %t127 = trunc i8 %t122 to i1
  br i1 %t127, label %gate.pass126, label %loop
  gate.pass126:
  br label %guard.end120
  guard.end120:
  %t130 = alloca i64
  %t131 = alloca i64
  %t132 = alloca i64
  %t135 = ptrtoint ptr %t130 to i64
  %t137 = ptrtoint ptr %t131 to i64
  %t139 = ptrtoint ptr %t132 to i64
  %t144 = add i64 0, 0
  %t145 = add i64 0, 0
  %t146 = add i64 0, 1
  %t147 = inttoptr i64 %t44 to ptr
  %t148 = inttoptr i64 %t47 to ptr
  %t133 = call i64 @vm_loop(i64 %t135, i64 %t137, i64 %t139, ptr %t147, i64 %t35, ptr %t148, i64 %t41, i64 %t144, i64 %t145, i64 %t146)
  %t149 = add i64 0, 0
  ret i64 %t149
}

define void @init_state(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #0 {
  entry:
  %ip_0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 0, ptr %ip_0, align 4
  ret void
}


define void @reactor_tick(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #2 {
  entry:
  ret void
}

define i32 @main() local_unnamed_addr #0 {
entry:
  %state = alloca %State, align 8
  %t0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 0, ptr %t0, align 4
  %state_save = alloca %State, align 8
  br label %.loop
.loop:
  call void @llvm.memcpy.p0p0i64(ptr %state_save, ptr %state, i64 8, i1 false)
  call void @reactor_tick(ptr noalias nocapture %state)
  br label %.end
.end:
  ret i32 0
}


attributes #0 = {
    mustprogress nofree norecurse nosync nounwind memory(readwrite)
}
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(readwrite) }
attributes #2 = { mustprogress nofree norecurse nosync nounwind memory(readwrite) }
attributes #3 = { nofree norecurse nosync nounwind memory(readwrite) }
attributes #6 = { nounwind }
attributes #7 = {
    mustprogress nofree norecurse nosync nounwind willreturn memory(read)
}
attributes #8 = {
    mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite)
}
attributes #9 = {
    nofree norecurse nosync nounwind memory(readwrite)
}
attributes #10 = {
    mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: read)
}
attributes #11 = {
    mustprogress nofree norecurse nosync nounwind memory(argmem: readwrite)
}
attributes #12 = {
    mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite)
}

!0 = !{!"Brief"}
!1 = !{!"Int", !0}
!2 = !{!"Bit", !0}
!3 = !{!"Bool", !0}
!4 = !{!"Char", !0}
!5 = !{!"Data", !0}
!6 = !{!"Double", !0}
!7 = !{!"FP128", !0}
!8 = !{!"Float", !0}
!9 = !{!"Float32", !0}
!10 = !{!"Float64", !0}
!11 = !{!"Half", !0}
!12 = !{!"BFloat", !0}
!13 = !{!"Int128", !0}
!14 = !{!"Int16", !0}
!15 = !{!"Int32", !0}
!16 = !{!"Int64", !0}
!17 = !{!"Int8", !0}
!18 = !{!"String", !0}
!19 = !{!"UInt", !0}
!20 = !{!"UInt128", !0}
!21 = !{!"UInt16", !0}
!22 = !{!"UInt32", !0}
!23 = !{!"UInt64", !0}
!24 = !{!"UInt8", !0}
!25 = !{!"Void", !0}
!26 = !{!"X86_FP80", !0}
!99 = distinct !{} ; StateAliasScope
