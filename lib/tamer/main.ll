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
declare i64 @__getenv_int(i128) #6
declare void @__print_str(i128) #6
declare i64 @__print_float(float) #6
declare i64 @__print_int(i64) #6
declare i128 @__getenv_brief(i128) #6
declare i64 @__print_char(i64) #6
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
  %t0 = load i64, ptr %t1, align 8
  %t6 = add i64 0, 4294967295
  %t4 = and i64 %t0, %t6
  %t9 = add i64 0, 1380532556
  %t10 = icmp ne i64 %t4, %t9
  %t7 = zext i1 %t10 to i8
  %t12 = trunc i8 %t7 to i1
  br i1 %t12, label %guard.then11, label %guard.end11
  guard.then11:
  %t13 = add i64 0, 101
  ret i64 %t13
  br label %guard.end11
  guard.end11:
  %t19 = add i64 0, 4
  %t17 = add nsw i64 %ac0, %t19
  %t16 = load i64, ptr %t17, align 8
  %t24 = add i64 0, 5
  %t22 = add nsw i64 %ac0, %t24
  %t21 = load i64, ptr %t22, align 8
  %t29 = add i64 0, 6
  %t27 = add nsw i64 %ac0, %t29
  %t26 = load i64, ptr %t27, align 8
  %t34 = add i64 0, 7
  %t32 = add nsw i64 %ac0, %t34
  %t31 = load i64, ptr %t32, align 8
  %t38 = add i64 0, 20
  %t36 = sdiv i64 %t21, %t38
  %t39 = add nsw i64 %ac0, %t26
  %t42 = add nsw i64 %ac0, %t16
  %t46 = add nsw i64 %t26, %t31
  %t50 = icmp sgt i64 %t46, %arg1
  %t45 = zext i1 %t50 to i8
  %t52 = trunc i8 %t45 to i1
  br i1 %t52, label %guard.then51, label %guard.end51
  guard.then51:
  %t53 = add i64 0, 103
  ret i64 %t53
  br label %guard.end51
  guard.end51:
  %t57 = add nsw i64 %t16, %t21
  %t61 = icmp sgt i64 %t57, %arg1
  %t56 = zext i1 %t61 to i8
  %t63 = trunc i8 %t56 to i1
  br i1 %t63, label %guard.then62, label %guard.end62
  guard.then62:
  %t64 = add i64 0, 104
  ret i64 %t64
  br label %guard.end62
  guard.end62:
  %t69 = add i64 0, 0
  %t70 = icmp eq i64 %t36, %t69
  %t67 = zext i1 %t70 to i8
  %t72 = trunc i8 %t67 to i1
  br i1 %t72, label %guard.then71, label %guard.end71
  guard.then71:
  %t73 = add i64 0, 105
  ret i64 %t73
  br label %guard.end71
  guard.end71:
  %t76 = call i64 @compute_buffer_sizes(i64 %t42, i64 %t36, i64 %t39, i64 %t31)
  %t83 = add i64 0, 1024
  %t84 = icmp sle i64 %t76, %t83
  %t81 = zext i1 %t84 to i8
  %t86 = trunc i8 %t81 to i1
  br i1 %t86, label %guard.then85, label %guard.end85
  guard.then85:
  %t89 = add i64 0, 0
  %t90 = icmp sge i64 %t76, %t89
  %t87 = zext i1 %t90 to i8
  %t92 = trunc i8 %t87 to i1
  br i1 %t92, label %gate.pass91, label %loop
  gate.pass91:
  br label %guard.end85
  guard.end85:
  %t96 = load i64, ptr @locals_slots
  %t97 = add i64 0, 4096
  %t98 = icmp sle i64 %t96, %t97
  %t95 = zext i1 %t98 to i8
  %t100 = trunc i8 %t95 to i1
  br i1 %t100, label %guard.then99, label %guard.end99
  guard.then99:
  %t102 = load i64, ptr @locals_slots
  %t103 = add i64 0, 0
  %t104 = icmp sge i64 %t102, %t103
  %t101 = zext i1 %t104 to i8
  %t106 = trunc i8 %t101 to i1
  br i1 %t106, label %gate.pass105, label %loop
  gate.pass105:
  br label %guard.end99
  guard.end99:
  %t110 = load i64, ptr @frames_max
  %t111 = add i64 0, 256
  %t112 = icmp sle i64 %t110, %t111
  %t109 = zext i1 %t112 to i8
  %t114 = trunc i8 %t109 to i1
  br i1 %t114, label %guard.then113, label %guard.end113
  guard.then113:
  %t116 = load i64, ptr @frames_max
  %t117 = add i64 0, 0
  %t118 = icmp sgt i64 %t116, %t117
  %t115 = zext i1 %t118 to i8
  %t120 = trunc i8 %t115 to i1
  br i1 %t120, label %gate.pass119, label %loop
  gate.pass119:
  br label %guard.end113
  guard.end113:
  %t123 = alloca i64
  %t124 = alloca i64
  %t125 = alloca i64
  %t128 = ptrtoint ptr %t123 to i64
  %t130 = ptrtoint ptr %t124 to i64
  %t132 = ptrtoint ptr %t125 to i64
  %t137 = add i64 0, 0
  %t138 = add i64 0, 0
  %t139 = add i64 0, 1
  %t126 = call i64 @vm_loop(i64 %t128, i64 %t130, i64 %t132, i64 %t39, i64 %t31, i64 %t42, i64 %t36, i64 %t137, i64 %t138, i64 %t139)
  %t140 = add i64 0, 0
  ret i64 %t140
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
