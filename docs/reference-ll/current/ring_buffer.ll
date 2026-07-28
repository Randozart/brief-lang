; ModuleID = 'program.ll'
source_filename = "program.bv"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

%SmallString64 = type { i64, i64, i64, i64, i64, i64, i64, i64, i64 }
%StaticString = type { i64, i64 }
%String = type { i64, i64 }
%UTF8View = type { i64, i64 }

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
declare i64 @__print_char(i64) #6
declare void @__print_str({ i64, i64 }) #6
declare i64 @__print_int(i64) #6
declare i64 @__getenv_int({ i64, i64 }) #6
declare { i64, i64 } @__getenv_brief({ i64, i64 }) #6
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
@CAP = constant i64 1024
@TOTAL = constant i64 50000000

%StateChunk0 = type { i64, i64, i64, i64, i64 }
%State = type { i64, i64, i64, i64, i64 }
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

define void @txn_enqueue(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t7 = load i64, ptr %t6, align 8, !range !50
  %t8 = load i64, ptr @TOTAL
  %t9 = icmp slt i64 %t7, %t8
  %t4 = zext i1 %t9 to i8
  %pi10 = trunc i8 %t4 to i1
  br i1 %pi10, label %ps12, label %pp11
  pp11:
    unreachable
  ps12:
  call void @llvm.assume(i1 %pi10)
  %t14 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t15 = load i64, ptr %t14, align 8, !range !50
  %t17 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t18 = load i64, ptr %t17, align 8
  %t21 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t22 = load i64, ptr %t21, align 8
  %t23 = load i64, ptr @CAP
  %t19 = srem i64 %t22, %t23
  %t24 = inttoptr i64 %t18 to ptr
  %t26 = add i64 %t19, 0
  %t25 = getelementptr i64, ptr %t24, i64 %t26
  store i64 %t15, ptr %t25
  %t29 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t30 = load i64, ptr %t29, align 8
  %t31 = add i64 0, 1
  %t27 = add nsw i64 %t30, %t31
  %t32 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 %t27, ptr %t32
  %t35 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t36 = load i64, ptr %t35, align 8, !range !50
  %t37 = add i64 0, 1
  %t33 = add nsw i64 %t36, %t37
  %t38 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 %t33, ptr %t38
  %t41 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t42 = load i64, ptr %t41, align 8, !range !50
  %t43 = load i64, ptr @TOTAL
  %t44 = icmp eq i64 %t42, %t43
  %t39 = zext i1 %t44 to i8
  %t46 = trunc i8 %t39 to i1
  br i1 %t46, label %guard.then45, label %guard.end45
  guard.then45:
  %t50 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t51 = load i64, ptr %t50, align 8
  %t52 = add i64 0, 0
  %t53 = inttoptr i64 %t51 to ptr
  %t54 = add i64 %t52, 0
  %t55 = getelementptr i64, ptr %t53, i64 %t54
  %t48 = load i64, ptr %t55
  %t58 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t59 = load i64, ptr %t58, align 8
  %t60 = add i64 0, 512
  %t61 = inttoptr i64 %t59 to ptr
  %t62 = add i64 %t60, 0
  %t63 = getelementptr i64, ptr %t61, i64 %t62
  %t56 = load i64, ptr %t63
  %t47 = add nsw i64 %t48, %t56
   %t65 = call i64 @__print_int(i64 %t47)
  %t68 = add i64 0, 10
   %t67 = call i64 @__print_char(i64 %t68)
  br label %guard.end45
  guard.end45:
  %t73 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t74 = load i64, ptr %t73, align 8, !range !50
  %t75 = add i64 0, 5000000
  %t71 = srem i64 %t74, %t75
  %t76 = add i64 0, 0
  %t77 = icmp eq i64 %t71, %t76
  %t70 = zext i1 %t77 to i8
  %t79 = trunc i8 %t70 to i1
  br i1 %t79, label %guard.then78, label %guard.end78
  guard.then78:
  %t82 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t83 = load i64, ptr %t82, align 8
  %t85 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t86 = load i64, ptr %t85, align 8
  %t80 = sub nsw i64 %t83, %t86
  %t89 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t90 = load i64, ptr %t89, align 8
  %t93 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t94 = load i64, ptr %t93, align 8
  %t95 = load i64, ptr @CAP
  %t91 = srem i64 %t94, %t95
  %t96 = inttoptr i64 %t90 to ptr
  %t97 = add i64 %t91, 0
  %t98 = getelementptr i64, ptr %t96, i64 %t97
  %t87 = load i64, ptr %t98
  %t101 = add nsw i64 %t80, %t87
   %t100 = call i64 @__print_int(i64 %t101)
  %t105 = add i64 0, 10
   %t104 = call i64 @__print_char(i64 %t105)
  br label %guard.end78
  guard.end78:
  ret void
}

define internal i8 @pre_enqueue(ptr noundef noalias nocapture align 8 %state) #10 {
  entry:
  %t2 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t3 = load i64, ptr %t2, align 8, !range !50
  %t4 = load i64, ptr @TOTAL
  %t5 = icmp slt i64 %t3, %t4
  %t0 = zext i1 %t5 to i8
  ret i8 %t0
}
define void @init_state(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #0 {
  entry:
  %ip_0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t2 = load i64, ptr @CAP
  %t3 = add i64 0, 8
  %t1 = mul nsw i64 %t2, %t3
  %t0_p = call ptr @malloc(i64 %t1)
  %t0 = ptrtoint ptr %t0_p to i64
   %t4 = add i64 %t1, 0
  store i64 %t0, ptr %ip_0, align 8
  %ip_1 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 0, ptr %ip_1, align 8
  %ip_2 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 0, ptr %ip_2, align 8
  %ip_3 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 0, ptr %ip_3, align 8
  %ip_4 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 0, ptr %ip_4, align 8
  ret void
}


define i32 @main() local_unnamed_addr #9 {
entry:
  %state = alloca %State, align 8
  %t5 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t8 = load i64, ptr @CAP
  %t9 = add i64 0, 8
  %t7 = mul nsw i64 %t8, %t9
  %t6_p = call ptr @malloc(i64 %t7)
  %t6 = ptrtoint ptr %t6_p to i64
   %t10 = add i64 %t7, 0
  store i64 %t6, ptr %t5, align 8
  %t11 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 0, ptr %t11, align 8
  %t12 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 0, ptr %t12, align 8
  %t13 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 0, ptr %t13, align 8
  %t14 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 0, ptr %t14, align 8
  %whb15 = add i64 0, 50000000
  br label %.wloop
.wloop:
  %t16 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t17 = load i64, ptr %t16, align 8, !range !50
  %whd18 = icmp slt i64 %t17, %whb15
  br i1 %whd18, label %.wbody, label %.wend
.wbody:
  %t20 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t21 = load i64, ptr %t20, align 8, !range !50
  %t23 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t24 = load i64, ptr %t23, align 8
  %t27 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t28 = load i64, ptr %t27, align 8
  %t29 = load i64, ptr @CAP
  %t25 = srem i64 %t28, %t29
  %t30 = inttoptr i64 %t24 to ptr
  %t32 = add i64 %t25, 0
  %t31 = getelementptr i64, ptr %t30, i64 %t32
  store i64 %t21, ptr %t31
  %t35 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t36 = load i64, ptr %t35, align 8
  %t37 = add i64 0, 1
  %t33 = add nsw i64 %t36, %t37
  %cms38 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 %t33, ptr %cms38, align 8
  %t41 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t42 = load i64, ptr %t41, align 8, !range !50
  %t43 = add i64 0, 1
  %t39 = add nsw i64 %t42, %t43
  %cms44 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 %t39, ptr %cms44, align 8
  %t48 = add i64 0, 5000000
  %t46 = srem i64 %t39, %t48
  %t49 = add i64 0, 0
  %t50 = icmp eq i64 %t46, %t49
  %t45 = zext i1 %t50 to i8
  %tb51 = trunc i8 %t45 to i1
  br i1 %tb51, label %.cmgb52, label %.cmgn52
.cmgb52:
  %t56 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t57 = load i64, ptr %t56, align 8
  %t53 = sub nsw i64 %t33, %t57
  %t60 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t61 = load i64, ptr %t60, align 8
  %t64 = load i64, ptr @CAP
  %t62 = srem i64 %t33, %t64
  %t65 = inttoptr i64 %t61 to ptr
  %t66 = add i64 %t62, 0
  %t67 = getelementptr i64, ptr %t65, i64 %t66
  %t58 = load i64, ptr %t67
  %t70 = add nsw i64 %t53, %t58
   %t69 = call i64 @__print_int(i64 %t70)
  %t74 = add i64 0, 10
   %t73 = call i64 @__print_char(i64 %t74)
  br label %.cmgn52
.cmgn52:
  %t77 = load i64, ptr @TOTAL
  %t78 = icmp eq i64 %t39, %t77
  %t75 = zext i1 %t78 to i8
  %tb79 = trunc i8 %t75 to i1
  br i1 %tb79, label %.cmgb80, label %.cmgn80
.cmgb80:
  %t84 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t85 = load i64, ptr %t84, align 8
  %t86 = add i64 0, 0
  %t87 = inttoptr i64 %t85 to ptr
  %t88 = add i64 %t86, 0
  %t89 = getelementptr i64, ptr %t87, i64 %t88
  %t82 = load i64, ptr %t89
  %t92 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t93 = load i64, ptr %t92, align 8
  %t94 = add i64 0, 512
  %t95 = inttoptr i64 %t93 to ptr
  %t96 = add i64 %t94, 0
  %t97 = getelementptr i64, ptr %t95, i64 %t96
  %t90 = load i64, ptr %t97
  %t81 = add nsw i64 %t82, %t90
   %t99 = call i64 @__print_int(i64 %t81)
  %t102 = add i64 0, 10
   %t101 = call i64 @__print_char(i64 %t102)
  br label %.cmgn80
.cmgn80:
  %whn103 = add nuw nsw i64 %t17, 1
  %t104 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 %whn103, ptr %t104, align 8
  br label %.wloop, !llvm.loop !100
  br label %.wloop
.wend:
  ret i32 0
}

; Loop metadata
!101 = !{!"llvm.loop.vectorize.enable", i1 true}
!102 = !{!"llvm.loop.align", i32 32}
!100 = !{!100, !101, !102}

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

!50 = !{ i64 -9223372036854775808, i64 50000000 }

!0 = !{!"Brief"}
!1 = !{!"Int", !0}
!2 = !{!"Bool", !0}
!3 = !{!"Char", !0}
!4 = !{!"Data", !0}
!5 = !{!"Double", !0}
!6 = !{!"FP128", !0}
!7 = !{!"Float", !0}
!8 = !{!"Float32", !0}
!9 = !{!"Float64", !0}
!10 = !{!"Half", !0}
!11 = !{!"BFloat", !0}
!12 = !{!"Int128", !0}
!13 = !{!"Int16", !0}
!14 = !{!"Int32", !0}
!15 = !{!"Int64", !0}
!16 = !{!"Int8", !0}
!17 = !{!"SmallString64", !0}
!18 = !{!"StaticString", !0}
!19 = !{!"String", !0}
!20 = !{!"UInt", !0}
!21 = !{!"UInt128", !0}
!22 = !{!"UInt16", !0}
!23 = !{!"UInt32", !0}
!24 = !{!"UInt64", !0}
!25 = !{!"UInt8", !0}
!26 = !{!"UTF8View", !0}
!27 = !{!"Void", !0}
!28 = !{!"X86_FP80", !0}
!99 = distinct !{} ; StateAliasScope
