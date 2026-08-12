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
declare i64 @briev_syscall(i64, i64, i64, i64, i64, i64, i64)
declare i64 @briev_sysconf(i64)
declare ptr @dlopen(ptr, i32) nounwind
declare ptr @dlsym(ptr, ptr) nounwind
declare i32 @dlclose(ptr) nounwind
declare i64 @briev_backtrace()
declare void @__print_str({ i64, i64 }) #6
declare i64 @__getenv_int({ i64, i64 }) #6
declare i64 @__print_int(i64) #6
declare { i64, i64 } @__getenv_briev({ i64, i64 }) #6
declare i64 @__print_float(float) #6
declare i64 @__print_char(i64) #6
declare i8* @__chr_to_str(i32) #1
declare i64 @__int_to_str__(i64) #1
declare i64 @__str_bytes__(i64) #1
declare i64 @__str_to_int(i8*) #1
declare i64 @briev_open(i64, i64, i64) #1
declare i64 @briev_close(i64) #1
declare i64 @briev_read(i64, i64, i64) #1
declare i64 @briev_write(i64, i64, i64) #1
declare i64 @briev_lseek(i64, i64, i64) #1
declare i64 @briev_pread(i64, i64, i64, i64) #1
declare i64 @briev_pwrite(i64, i64, i64, i64) #1
declare i64 @briev_stat(i64, i64) #1
declare i64 @briev_fstat(i64) #1
declare i64 @briev_truncate(i64, i64) #1
declare i64 @briev_ftruncate(i64, i64) #1
declare i64 @briev_fsync(i64) #1
declare i64 @briev_dup(i64) #1
declare i64 @briev_dup2(i64, i64) #1
declare i64 @briev_fcntl(i64, i64, i64) #1
declare i64 @briev_socket(i64, i64, i64) #1
declare i64 @briev_bind(i64, i64, i64) #1
declare i64 @briev_listen(i64, i64) #1
declare i64 @briev_accept(i64, i64, i64) #1
declare i64 @briev_connect(i64, i64, i64) #1
declare i64 @briev_send(i64, i64, i64, i64) #1
declare i64 @briev_recv(i64, i64, i64, i64) #1
declare i64 @briev_sendto(i64, i64, i64, i64, i64, i64) #1
declare i64 @briev_recvfrom(i64, i64, i64, i64, i64, i64) #1
declare i64 @briev_setsockopt(i64, i64, i64, i64, i64) #1
declare i64 @briev_getsockopt(i64, i64, i64, i64, i64) #1
declare i64 @briev_shutdown(i64, i64) #1
declare i64 @briev_mkdir(i64, i64) #1
declare i64 @briev_rmdir(i64) #1
declare i64 @briev_unlink(i64) #1
declare i64 @briev_rename(i64, i64) #1
declare i64 @briev_symlink(i64, i64) #1
declare i64 @briev_link(i64, i64) #1
declare i64 @briev_chdir(i64) #1
declare i64 @briev_chmod(i64, i64) #1
declare i64 @briev_chown(i64, i64, i64) #1
declare i64 @briev_umask(i64) #1
declare i64 @briev_access(i64, i64) #1
declare i64 @briev_mmap(i64, i64, i64, i64, i64, i64) #1
declare i64 @briev_munmap(i64, i64) #1
declare i64 @briev_mprotect(i64, i64, i64) #1
declare i64 @briev_brk(i64) #1
declare i64 @briev_mlock(i64, i64) #1
declare i64 @briev_pipe(i64) #1
declare i64 @briev_shm_open(i64, i64, i64) #1
declare i64 @briev_shm_unlink(i64) #1
declare i64 @briev_sem_open(i64, i64, i64, i64) #1
declare i64 @briev_sem_wait(i64) #1
declare i64 @briev_sem_post(i64) #1
declare i64 @briev_getpid() #1
declare i64 @briev_getppid() #1
declare i64 @briev_clock_gettime(i64, i64) #1
declare i64 @briev_nanosleep(i64, i64) #1
declare i64 @briev_getenv(i64, i64, i64) #1
declare i64 @briev_setenv(i64, i64, i64) #1
declare i64 @briev_unsetenv(i64) #1
declare i64 @briev_futex(i64, i64, i64, i64, i64, i64) #1
declare i64 @__ioctl__(i64, i64, i64) #1
declare i64 @__isatty__(i64) #1
declare i64 @__print(i64) #1
declare i64 @briev_getuid() #1
declare i64 @briev_geteuid() #1
declare i64 @briev_getgid() #1
declare i64 @briev_getegid() #1
declare i64 @briev_sched_yield() #1
declare i64 @briev_getpriority(i64, i64) #1
declare i64 @briev_setpriority(i64, i64, i64) #1
declare i64 @briev_getrlimit(i64) #1
declare i64 @briev_setrlimit(i64, i64) #1
declare i64 @briev_pagesize() #1
declare i64 @briev_cpu_count() #1
declare i64 @briev_ttyname(i64) #1
declare i64 @briev_ring_push(i64, i64) #1
declare i64 @briev_ring_pop(i64) #1
declare i64 @__tty_read_key__(i64) #1
declare i64 @__tty_size__() #1
declare i64 @cpu_count() #1
declare i64 @pagesize() #1
@STR_READFILE_ERR = private unnamed_addr constant [15 x i8] c"file not found\00"
@stdout = external dso_local global ptr
declare void @__wait_for_trigger__() #1
%StateChunk0 = type { i64, i64, i64 }
%State = type { i64, i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant <{ i64, [1 x i8] }> <{ i64 0, [1 x i8] c"\00" }>, align 8
@str.1 = private unnamed_addr constant <{ i64, [6 x i8] }> <{ i64 5, [6 x i8] c"BOUND\00" }>, align 8

@ll_empty_list = private unnamed_addr constant { i64, i64 } { i64 0, i64 0 }

define ptr @get_env(ptr noundef noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
    %t2 = inttoptr i64 %ac0 to ptr
   %t0 = call ptr @__getenv_briev(ptr %t2)
  ret ptr %t0
}

define i64 @get_env_int(ptr noundef noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
    %t2 = inttoptr i64 %ac0 to ptr
   %t0 = call i64 @__getenv_int(ptr %t2)
  ret i64 %t0
}

define void @txn_work(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #11 alwaysinline {
  entry:
  %t7 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t8 = load i64, ptr %t7, align 8
  %t9 = add i64 0, 0
  %t10 = icmp sgt i64 %t8, %t9
  %t5 = zext i1 %t10 to i8
  %t13 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t14 = load i64, ptr %t13, align 8
  %t16 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t17 = load i64, ptr %t16, align 8
  %t18 = icmp slt i64 %t14, %t17
  %t11 = zext i1 %t18 to i8
  %t4 = and i8 %t5, %t11
  %pi19 = trunc i8 %t4 to i1
  br i1 %pi19, label %ps21, label %pp20
  pp20:
    unreachable
  ps21:
  call void @llvm.assume(i1 %pi19)
  %t24 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t25 = load i64, ptr %t24, align 8
  %t26 = add i64 0, 1
  %t22 = add nsw i64 %t25, %t26
  %t27 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 %t22, ptr %t27
  %t31 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t32 = load i64, ptr %t31, align 8
  %t33 = add i64 0, 100000
  %t29 = srem i64 %t32, %t33
  %t34 = add i64 0, 0
  %t35 = icmp eq i64 %t29, %t34
  %t28 = zext i1 %t35 to i8
  %t37 = trunc i8 %t28 to i1
  br i1 %t37, label %guard.then36, label %guard.end36
  guard.then36:
    %t38 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
    %t39 = load i64, ptr %t38
    call void @txn_work_cold_0(i64 %t39)
    br label %guard.end36
  guard.end36:
  ret void
}
define void @txn_work_cold_0(i64 %__cp_ops) local_unnamed_addr #0 {
   %t42 = call i64 @__print_int(i64 %__cp_ops)
  %t45 = add i64 0, 10
   %t44 = call i64 @__print_char(i64 %t45)
  ret void
}


define internal i8 @pre_work(ptr noundef noalias nocapture align 8 %state) #10 {
  entry:
  %t3 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t4 = load i64, ptr %t3, align 8
  %t5 = add i64 0, 0
  %t6 = icmp sgt i64 %t4, %t5
  %t1 = zext i1 %t6 to i8
  %t9 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t10 = load i64, ptr %t9, align 8
  %t12 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t13 = load i64, ptr %t12, align 8
  %t14 = icmp slt i64 %t10, %t13
  %t7 = zext i1 %t14 to i8
  %t0 = and i8 %t1, %t7
  ret i8 %t0
}
define void @init_state(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #0 {
  entry:
  %ip_0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t2 = bitcast <{ i64, [6 x i8] }>* @str.1 to ptr
  %t3 = ptrtoint ptr %t2 to i64
  %t4 = inttoptr i64 %t3 to ptr
  %t0 = call i64 @get_env_int(ptr %state, ptr %t4)
  store i64 %t0, ptr %ip_0, align 8
  %ip_1 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 0, ptr %ip_1, align 8
  %ip_2 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 0, ptr %ip_2, align 8
  ret void
}


define i32 @main() local_unnamed_addr #9 {
entry:
  %state = alloca %State, align 8
  %t5 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t8 = bitcast <{ i64, [6 x i8] }>* @str.1 to ptr
  %t9 = ptrtoint ptr %t8 to i64
  %t10 = inttoptr i64 %t9 to ptr
  %t6 = call i64 @get_env_int(ptr %state, ptr %t10)
  store i64 %t6, ptr %t5, align 8
  %t11 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 0, ptr %t11, align 8
  %t12 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 0, ptr %t12, align 8
  %clb14 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %whb13 = load i64, ptr %clb14, align 8
  br label %.wloop
.wloop:
  %t15 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t16 = load i64, ptr %t15, align 8
  %whd17 = icmp slt i64 %t16, %whb13
  br i1 %whd17, label %.wbody, label %.wend
.wbody:
  %t20 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t21 = load i64, ptr %t20, align 8
  %t22 = add i64 0, 1
  %t18 = add nsw i64 %t21, %t22
  %cms23 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 %t18, ptr %cms23, align 8
  %t27 = add i64 0, 100000
  %t25 = srem i64 %t18, %t27
  %t28 = add i64 0, 0
  %t29 = icmp eq i64 %t25, %t28
  %t24 = zext i1 %t29 to i8
  %tb30 = trunc i8 %t24 to i1
  br i1 %tb30, label %.cmgb31, label %.cmgn31
.cmgb31:
   %t33 = call i64 @__print_int(i64 %t18)
  %t36 = add i64 0, 10
   %t35 = call i64 @__print_char(i64 %t36)
  br label %.cmgn31
.cmgn31:
  %whn37 = add nuw nsw i64 %t16, 1
  %t38 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 %whn37, ptr %t38, align 8
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

!0 = !{!"Briev"}
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
