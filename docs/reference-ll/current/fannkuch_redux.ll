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
declare i64 @briv_syscall(i64, i64, i64, i64, i64, i64, i64)
declare i64 @briv_sysconf(i64)
declare ptr @dlopen(ptr, i32) nounwind
declare ptr @dlsym(ptr, ptr) nounwind
declare i32 @dlclose(ptr) nounwind
declare i64 @briv_backtrace()
declare i64 @__print_char(i64) #6
declare i64 @__print_float(float) #6
declare i64 @__print_int(i64) #6
declare void @__print_str({ i64, i64 }) #6
declare { i64, i64 } @__getenv_briv({ i64, i64 }) #6
declare i64 @__getenv_int({ i64, i64 }) #6
declare i8* @__chr_to_str(i32) #1
declare i64 @__int_to_str__(i64) #1
declare i64 @__str_bytes__(i64) #1
declare i64 @__str_to_int(i8*) #1
declare i64 @briv_open(i64, i64, i64) #1
declare i64 @briv_close(i64) #1
declare i64 @briv_read(i64, i64, i64) #1
declare i64 @briv_write(i64, i64, i64) #1
declare i64 @briv_lseek(i64, i64, i64) #1
declare i64 @briv_pread(i64, i64, i64, i64) #1
declare i64 @briv_pwrite(i64, i64, i64, i64) #1
declare i64 @briv_stat(i64, i64) #1
declare i64 @briv_fstat(i64) #1
declare i64 @briv_truncate(i64, i64) #1
declare i64 @briv_ftruncate(i64, i64) #1
declare i64 @briv_fsync(i64) #1
declare i64 @briv_dup(i64) #1
declare i64 @briv_dup2(i64, i64) #1
declare i64 @briv_fcntl(i64, i64, i64) #1
declare i64 @briv_socket(i64, i64, i64) #1
declare i64 @briv_bind(i64, i64, i64) #1
declare i64 @briv_listen(i64, i64) #1
declare i64 @briv_accept(i64, i64, i64) #1
declare i64 @briv_connect(i64, i64, i64) #1
declare i64 @briv_send(i64, i64, i64, i64) #1
declare i64 @briv_recv(i64, i64, i64, i64) #1
declare i64 @briv_sendto(i64, i64, i64, i64, i64, i64) #1
declare i64 @briv_recvfrom(i64, i64, i64, i64, i64, i64) #1
declare i64 @briv_setsockopt(i64, i64, i64, i64, i64) #1
declare i64 @briv_getsockopt(i64, i64, i64, i64, i64) #1
declare i64 @briv_shutdown(i64, i64) #1
declare i64 @briv_mkdir(i64, i64) #1
declare i64 @briv_rmdir(i64) #1
declare i64 @briv_unlink(i64) #1
declare i64 @briv_rename(i64, i64) #1
declare i64 @briv_symlink(i64, i64) #1
declare i64 @briv_link(i64, i64) #1
declare i64 @briv_chdir(i64) #1
declare i64 @briv_chmod(i64, i64) #1
declare i64 @briv_chown(i64, i64, i64) #1
declare i64 @briv_umask(i64) #1
declare i64 @briv_access(i64, i64) #1
declare i64 @briv_mmap(i64, i64, i64, i64, i64, i64) #1
declare i64 @briv_munmap(i64, i64) #1
declare i64 @briv_mprotect(i64, i64, i64) #1
declare i64 @briv_brk(i64) #1
declare i64 @briv_mlock(i64, i64) #1
declare i64 @briv_pipe(i64) #1
declare i64 @briv_shm_open(i64, i64, i64) #1
declare i64 @briv_shm_unlink(i64) #1
declare i64 @briv_sem_open(i64, i64, i64, i64) #1
declare i64 @briv_sem_wait(i64) #1
declare i64 @briv_sem_post(i64) #1
declare i64 @briv_getpid() #1
declare i64 @briv_getppid() #1
declare i64 @briv_clock_gettime(i64, i64) #1
declare i64 @briv_nanosleep(i64, i64) #1
declare i64 @briv_getenv(i64, i64, i64) #1
declare i64 @briv_setenv(i64, i64, i64) #1
declare i64 @briv_unsetenv(i64) #1
declare i64 @briv_futex(i64, i64, i64, i64, i64, i64) #1
declare i64 @__ioctl__(i64, i64, i64) #1
declare i64 @__isatty__(i64) #1
declare i64 @__print(i64) #1
declare i64 @briv_getuid() #1
declare i64 @briv_geteuid() #1
declare i64 @briv_getgid() #1
declare i64 @briv_getegid() #1
declare i64 @briv_sched_yield() #1
declare i64 @briv_getpriority(i64, i64) #1
declare i64 @briv_setpriority(i64, i64, i64) #1
declare i64 @briv_getrlimit(i64) #1
declare i64 @briv_setrlimit(i64, i64) #1
declare i64 @briv_pagesize() #1
declare i64 @briv_cpu_count() #1
declare i64 @briv_ttyname(i64) #1
declare i64 @briv_ring_push(i64, i64) #1
declare i64 @briv_ring_pop(i64) #1
declare i64 @__tty_read_key__(i64) #1
declare i64 @__tty_size__() #1
declare i64 @cpu_count() #1
declare i64 @pagesize() #1
@STR_READFILE_ERR = private unnamed_addr constant [15 x i8] c"file not found\00"
@stdout = external dso_local global ptr
declare void @__wait_for_trigger__() #1
@IA = constant i64 3877
@IC = constant i64 29573
@IM = constant i64 139968

%StateChunk0 = type { i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64 }
%StateChunk1 = type { i64, i64, i64 }
%State = type { i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant <{ i64, [1 x i8] }> <{ i64 0, [1 x i8] c"\00" }>, align 8
@str.1 = private unnamed_addr constant <{ i64, [6 x i8] }> <{ i64 5, [6 x i8] c"BOUND\00" }>, align 8

@ll_empty_list = private unnamed_addr constant { i64, i64 } { i64 0, i64 0 }

define ptr @get_env(ptr noundef noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
    %t2 = inttoptr i64 %ac0 to ptr
   %t0 = call ptr @__getenv_briv(ptr %t2)
  ret ptr %t0
}

define i64 @get_env_int(ptr noundef noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
    %t2 = inttoptr i64 %ac0 to ptr
   %t0 = call i64 @__getenv_int(ptr %t2)
  ret i64 %t0
}

define void @txn_fan(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #11 alwaysinline {
  entry:
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t7 = load i64, ptr %t6, align 8
  %t9 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t10 = load i64, ptr %t9, align 8
  %t11 = icmp slt i64 %t7, %t10
  %t4 = zext i1 %t11 to i8
  %pi12 = trunc i8 %t4 to i1
  br i1 %pi12, label %ps14, label %pp13
  pp13:
    unreachable
  ps14:
  call void @llvm.assume(i1 %pi12)
  %t19 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t20 = load i64, ptr %t19, align 8
  %t21 = load i64, ptr @IA
  %t17 = mul nsw i64 %t20, %t21
  %t22 = load i64, ptr @IC
  %t16 = add nsw i64 %t17, %t22
  %t23 = load i64, ptr @IM
  %t15 = srem i64 %t16, %t23
  %t25 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t26 = load i64, ptr %t25, align 8
  %t29 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t30 = load i64, ptr %t29, align 8
  %t31 = add i64 0, 1
  %t27 = add nsw i64 %t30, %t31
  %t32 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t27, ptr %t32
  %t34 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 %t15, ptr %t34
  %t37 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t38 = load i64, ptr %t37, align 8
  %t41 = add i64 0, 13
  %t39 = srem i64 %t26, %t41
  %t35 = add nsw i64 %t38, %t39
  %t43 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t44 = load i64, ptr %t43, align 8
  %t45 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store i64 %t44, ptr %t45
  %t48 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t49 = load i64, ptr %t48, align 8
  %t52 = add i64 0, 17
  %t50 = srem i64 %t35, %t52
  %t46 = add nsw i64 %t49, %t50
  %t54 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 %t35, ptr %t54
  %t56 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t57 = load i64, ptr %t56, align 8
  %t58 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 %t57, ptr %t58
  %t60 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 %t46, ptr %t60
  %t63 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t64 = load i64, ptr %t63, align 8
  %t66 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t67 = load i64, ptr %t66, align 8
  %t68 = icmp eq i64 %t64, %t67
  %t61 = zext i1 %t68 to i8
  %t70 = trunc i8 %t61 to i1
  br i1 %t70, label %guard.then69, label %guard.end69
  guard.then69:
    %t71 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
    %t72 = load i64, ptr %t71
    call void @txn_fan_cold_0(i64 %t72)
    br label %guard.end69
  guard.end69:
  %t74 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t75 = load i64, ptr %t74, align 8
  %t76 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store i64 %t75, ptr %t76
  %t78 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t79 = load i64, ptr %t78, align 8
  %t80 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store i64 %t79, ptr %t80
  %t82 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t83 = load i64, ptr %t82, align 8
  %t84 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store i64 %t83, ptr %t84
  %t86 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t87 = load i64, ptr %t86, align 8
  %t88 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store i64 %t87, ptr %t88
  %t90 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t91 = load i64, ptr %t90, align 8
  %t92 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  store i64 %t91, ptr %t92
  %t94 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t95 = load i64, ptr %t94, align 8
  %t96 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store i64 %t95, ptr %t96
  %t98 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t99 = load i64, ptr %t98, align 8
  %t100 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  store i64 %t99, ptr %t100
  %t102 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t103 = load i64, ptr %t102, align 8
  %t104 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  store i64 %t103, ptr %t104
  %t106 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t107 = load i64, ptr %t106, align 8
  %t108 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  store i64 %t107, ptr %t108
  %t110 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  store i64 %t26, ptr %t110
  ret void
}
define void @txn_fan_cold_0(i64 %__cp_checksum) local_unnamed_addr #0 {
   %t113 = call i64 @__print_int(i64 %__cp_checksum)
  %t116 = add i64 0, 10
   %t115 = call i64 @__print_char(i64 %t116)
  ret void
}


define internal i8 @pre_fan(ptr noundef noalias nocapture align 8 %state) #10 {
  entry:
  %t2 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = load i64, ptr %t2, align 8
  %t5 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t6 = load i64, ptr %t5, align 8
  %t7 = icmp slt i64 %t3, %t6
  %t0 = zext i1 %t7 to i8
  ret i8 %t0
}
define void @init_state(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #0 {
  entry:
  %ip_0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 0, ptr %ip_0, align 8
  %ip_1 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t2 = bitcast <{ i64, [6 x i8] }>* @str.1 to ptr
  %t3 = ptrtoint ptr %t2 to i64
  %t4 = inttoptr i64 %t3 to ptr
  %t0 = call i64 @get_env_int(ptr %state, ptr %t4)
  store i64 %t0, ptr %ip_1, align 8
  %ip_2 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 42, ptr %ip_2, align 8
  %ip_3 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 0, ptr %ip_3, align 8
  %ip_4 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 0, ptr %ip_4, align 8
  %ip_5 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store i64 0, ptr %ip_5, align 8
  %ip_6 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 1, ptr %ip_6, align 8
  %ip_7 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store i64 2, ptr %ip_7, align 8
  %ip_8 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store i64 3, ptr %ip_8, align 8
  %ip_9 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store i64 4, ptr %ip_9, align 8
  %ip_10 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store i64 5, ptr %ip_10, align 8
  %ip_11 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  store i64 6, ptr %ip_11, align 8
  %ip_12 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store i64 7, ptr %ip_12, align 8
  %ip_13 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  store i64 8, ptr %ip_13, align 8
  %ip_14 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  store i64 9, ptr %ip_14, align 8
  %ip_15 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  store i64 10, ptr %ip_15, align 8
  %ip_16 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  store i64 11, ptr %ip_16, align 8
  %ip_17 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
  store i64 0, ptr %ip_17, align 8
  ret void
}


define i32 @main() local_unnamed_addr #9 {
entry:
  %state = alloca %State, align 8
  %t5 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 0, ptr %t5, align 8
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t9 = bitcast <{ i64, [6 x i8] }>* @str.1 to ptr
  %t10 = ptrtoint ptr %t9 to i64
  %t11 = inttoptr i64 %t10 to ptr
  %t7 = call i64 @get_env_int(ptr %state, ptr %t11)
  store i64 %t7, ptr %t6, align 8
  %t12 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 42, ptr %t12, align 8
  %t13 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 0, ptr %t13, align 8
  %t14 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 0, ptr %t14, align 8
  %t15 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store i64 0, ptr %t15, align 8
  %t16 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 1, ptr %t16, align 8
  %t17 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store i64 2, ptr %t17, align 8
  %t18 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store i64 3, ptr %t18, align 8
  %t19 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store i64 4, ptr %t19, align 8
  %t20 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store i64 5, ptr %t20, align 8
  %t21 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  store i64 6, ptr %t21, align 8
  %t22 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store i64 7, ptr %t22, align 8
  %t23 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  store i64 8, ptr %t23, align 8
  %t24 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  store i64 9, ptr %t24, align 8
  %t25 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  store i64 10, ptr %t25, align 8
  %t26 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  store i64 11, ptr %t26, align 8
  %t27 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
  store i64 0, ptr %t27, align 8
  %clb29 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %fmb28 = load i64, ptr %clb29, align 8
  br label %.fm_loop
.fm_loop:
  %t30 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t31 = load i64, ptr %t30, align 8
  %fmd32 = icmp slt i64 %t31, %fmb28
  br i1 %fmd32, label %.fm_body, label %.fm_end
.fm_body:
  %t37 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t38 = load i64, ptr %t37, align 8
  %t39 = load i64, ptr @IA
  %t35 = mul nsw i64 %t38, %t39
  %t40 = load i64, ptr @IC
  %t34 = add nsw i64 %t35, %t40
  %t41 = load i64, ptr @IM
  %t33 = srem i64 %t34, %t41
  %t43 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t44 = load i64, ptr %t43, align 8
  %t47 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t48 = load i64, ptr %t47, align 8
  %t51 = add i64 0, 13
  %t49 = srem i64 %t44, %t51
  %t45 = add nsw i64 %t48, %t49
  %t54 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t55 = load i64, ptr %t54, align 8
  %t58 = add i64 0, 17
  %t56 = srem i64 %t45, %t58
  %t52 = add nsw i64 %t55, %t56
  %cms60 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 %t33, ptr %cms60, align 8
  %t62 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t63 = load i64, ptr %t62, align 8
  %cms64 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store i64 %t63, ptr %cms64, align 8
  %t66 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t67 = load i64, ptr %t66, align 8
  %cms68 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 %t67, ptr %cms68, align 8
  %t70 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t71 = load i64, ptr %t70, align 8
  %cms72 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store i64 %t71, ptr %cms72, align 8
  %t74 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t75 = load i64, ptr %t74, align 8
  %cms76 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store i64 %t75, ptr %cms76, align 8
  %t78 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t79 = load i64, ptr %t78, align 8
  %cms80 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store i64 %t79, ptr %cms80, align 8
  %t82 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t83 = load i64, ptr %t82, align 8
  %cms84 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store i64 %t83, ptr %cms84, align 8
  %t86 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t87 = load i64, ptr %t86, align 8
  %cms88 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  store i64 %t87, ptr %cms88, align 8
  %t90 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t91 = load i64, ptr %t90, align 8
  %cms92 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store i64 %t91, ptr %cms92, align 8
  %t94 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t95 = load i64, ptr %t94, align 8
  %cms96 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  store i64 %t95, ptr %cms96, align 8
  %t98 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t99 = load i64, ptr %t98, align 8
  %cms100 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  store i64 %t99, ptr %cms100, align 8
  %t102 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t103 = load i64, ptr %t102, align 8
  %cms104 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  store i64 %t103, ptr %cms104, align 8
  %cms106 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  store i64 %t44, ptr %cms106, align 8
  %cms108 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 %t45, ptr %cms108, align 8
  %cms110 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 %t52, ptr %cms110, align 8
  %t113 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t114 = load i64, ptr %t113, align 8
  %t115 = add i64 0, 1
  %t111 = add nsw i64 %t114, %t115
  %cms116 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t111, ptr %cms116, align 8
  %fmn117 = add nuw nsw i64 %t31, 1
  %t118 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %fmn117, ptr %t118, align 8
  br label %.fm_loop, !llvm.loop !100
.fm_end:
  %t122 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t123 = load i64, ptr %t122, align 8
   %t120 = call i64 @__print_int(i64 %t123)
  %t125 = add i64 0, 10
   %t124 = call i64 @__print_char(i64 %t125)
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

!0 = !{!"Briv"}
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
