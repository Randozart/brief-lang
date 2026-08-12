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
declare i64 @__getenv_int({ i64, i64 }) #6
declare { i64, i64 } @__getenv_briev({ i64, i64 }) #6
declare void @__print_str({ i64, i64 }) #6
declare i64 @__print_char(i64) #6
declare i64 @__print_float(float) #6
declare i64 @__print_int(i64) #6
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
@R1 = constant i64 200
@R2 = constant i64 199

%StateChunk0 = type { i64, i64, i64, i64 }
%State = type { i64, i64, i64, i64 }
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

define void @txn_step(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #11 alwaysinline {
  entry:
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t7 = load i64, ptr %t6, align 8
  %t9 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t10 = load i64, ptr %t9, align 8
  %t11 = icmp slt i64 %t7, %t10
  %t4 = zext i1 %t11 to i8
  %pi12 = trunc i8 %t4 to i1
  br i1 %pi12, label %ps14, label %pp13
  pp13:
    unreachable
  ps14:
  call void @llvm.assume(i1 %pi12)
  %t17 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t18 = load i64, ptr %t17, align 8
  %t20 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t21 = load i64, ptr %t20, align 8
  %t15 = add nsw i64 %t18, %t21
  %t22 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 %t15, ptr %t22
  %t26 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t27 = load i64, ptr %t26, align 8
  %t28 = load i64, ptr @R1
  %t24 = add nsw i64 %t27, %t28
  %t29 = load i64, ptr @R2
  %t23 = sub nsw i64 %t24, %t29
  %t30 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 %t23, ptr %t30
  %t34 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t35 = load i64, ptr %t34, align 8
  %t36 = add i64 0, 5000000
  %t32 = srem i64 %t35, %t36
  %t37 = add i64 0, 0
  %t38 = icmp eq i64 %t32, %t37
  %t31 = zext i1 %t38 to i8
  %t40 = trunc i8 %t31 to i1
  br i1 %t40, label %guard.then39, label %guard.end39
  guard.then39:
    %t41 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
    %t42 = load i64, ptr %t41
    call void @txn_step_cold_0(i64 %t42)
    br label %guard.end39
  guard.end39:
  ret void
}
define void @txn_step_cold_0(i64 %__cp_acc) local_unnamed_addr #0 {
   %t45 = call i64 @__print_int(i64 %__cp_acc)
  %t48 = add i64 0, 10
   %t47 = call i64 @__print_char(i64 %t48)
  ret void
}


define internal i8 @pre_step(ptr noundef noalias nocapture align 8 %state) #10 {
  entry:
  %t2 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t3 = load i64, ptr %t2, align 8
  %t5 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t6 = load i64, ptr %t5, align 8
  %t7 = icmp slt i64 %t3, %t6
  %t0 = zext i1 %t7 to i8
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
  %ip_3 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 0, ptr %ip_3, align 8
  ret void
}


define i32 @main() local_unnamed_addr #9 {
entry:
  %state = alloca %State, align 8
  %t0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = bitcast <{ i64, [6 x i8] }>* @str.1 to ptr
  %t4 = ptrtoint ptr %t3 to i64
  %t5 = inttoptr i64 %t4 to ptr
  %t1 = call i64 @get_env_int(ptr %state, ptr %t5)
  store i64 %t1, ptr %t0, align 8
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 0, ptr %t6, align 8
  %t7 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 0, ptr %t7, align 8
  %t8 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 0, ptr %t8, align 8
  %sa9 = alloca i64, align 8
  %sa10 = alloca i64, align 8
  %sa11 = alloca i64, align 8
  %sa12 = alloca i64, align 8
  %any_active_13 = alloca i64, align 8
  br label %.ss_main_loop
.ss_main_loop:
  store i64 0, ptr %any_active_13
  %t16 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t17 = load i64, ptr %t16, align 8
  %t19 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t20 = load i64, ptr %t19, align 8
  %t21 = icmp slt i64 %t17, %t20
  %t14 = zext i1 %t21 to i8
  %tb22 = trunc i8 %t14 to i1
  br i1 %tb22, label %.ssb_step, label %.ssn_step
.ssb_step:
  store i64 1, ptr %any_active_13
  %t26 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t27 = load i64, ptr %t26, align 8
  %t29 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t30 = load i64, ptr %t29, align 8
  %t24 = add nsw i64 %t27, %t30
  %t31 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 %t24, ptr %t31
  %t35 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t36 = load i64, ptr %t35, align 8
  %t37 = load i64, ptr @R1
  %t33 = add nsw i64 %t36, %t37
  %t38 = load i64, ptr @R2
  %t32 = sub nsw i64 %t33, %t38
  %t39 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 %t32, ptr %t39
  %t43 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t44 = load i64, ptr %t43, align 8
  %t45 = add i64 0, 5000000
  %t41 = srem i64 %t44, %t45
  %t46 = add i64 0, 0
  %t47 = icmp eq i64 %t41, %t46
  %t40 = zext i1 %t47 to i8
  %t49 = trunc i8 %t40 to i1
  br i1 %t49, label %guard.then48, label %guard.end48
  guard.then48:
  %t53 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t54 = load i64, ptr %t53, align 8
   %t51 = call i64 @__print_int(i64 %t54)
  %t56 = add i64 0, 10
   %t55 = call i64 @__print_char(i64 %t56)
  br label %guard.end48
  guard.end48:
  br label %.ssn_step
.ssn_step:
  %t59 = load i64, ptr %any_active_13
  %t60 = icmp eq i64 %t59, 0
  br i1 %t60, label %.end, label %.ss_main_loop
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
