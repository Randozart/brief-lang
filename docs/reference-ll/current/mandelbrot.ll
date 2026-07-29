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
declare i64 @__print_float(float) #6
declare i64 @__print_char(i64) #6
declare i64 @__print_int(i64) #6
declare { i64, i64 } @__getenv_brief({ i64, i64 }) #6
declare void @__print_str({ i64, i64 }) #6
declare i64 @__getenv_int({ i64, i64 }) #6
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
@IA = constant i64 3877
@IC = constant i64 29573
@IM = constant i64 139968
@SCALE = constant i64 100
@THRESH = constant i64 40000

%StateChunk0 = type { i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64 }
%State = type { i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant <{ i64, [1 x i8] }> <{ i64 0, [1 x i8] c"\00" }>, align 8
@str.1 = private unnamed_addr constant <{ i64, [6 x i8] }> <{ i64 5, [6 x i8] c"BOUND\00" }>, align 8

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

define void @txn_mb(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #0 alwaysinline {
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
  %t27 = add i64 0, 200
  %t25 = srem i64 %t15, %t27
  %t28 = add i64 0, 100
  %t24 = sub nsw i64 %t25, %t28
  %t33 = load i64, ptr @IA
  %t31 = mul nsw i64 %t15, %t33
  %t34 = load i64, ptr @IC
  %t30 = add nsw i64 %t31, %t34
  %t35 = load i64, ptr @IM
  %t29 = srem i64 %t30, %t35
  %t39 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t40 = load i64, ptr %t39, align 8
  %t37 = mul nsw i64 %t40, %t24
  %t42 = load i64, ptr @SCALE
  %t36 = sdiv i64 %t37, %t42
  %t44 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store i64 %t24, ptr %t44
  %t48 = add i64 0, 200
  %t46 = srem i64 %t29, %t48
  %t49 = add i64 0, 100
  %t45 = sub nsw i64 %t46, %t49
  %t51 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 %t29, ptr %t51
  %t53 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store i64 %t36, ptr %t53
  %t57 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t58 = load i64, ptr %t57, align 8
  %t55 = mul nsw i64 %t58, %t45
  %t60 = load i64, ptr @SCALE
  %t54 = sdiv i64 %t55, %t60
  %t62 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 %t45, ptr %t62
  %t66 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t67 = load i64, ptr %t66, align 8
  %t64 = mul nsw i64 %t67, %t45
  %t69 = load i64, ptr @SCALE
  %t63 = sdiv i64 %t64, %t69
  %t75 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t76 = load i64, ptr %t75, align 8
  %t73 = mul nsw i64 %t76, %t24
  %t78 = load i64, ptr @SCALE
  %t72 = sdiv i64 %t73, %t78
  %t70 = add nsw i64 %t54, %t72
  %t80 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store i64 %t54, ptr %t80
  %t81 = sub nsw i64 %t36, %t63
  %t85 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store i64 %t63, ptr %t85
  %t87 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 %t70, ptr %t87
  %t89 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 %t81, ptr %t89
  %t93 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t94 = load i64, ptr %t93, align 8
  %t96 = mul nsw i64 %t81, %t81
  %t99 = load i64, ptr @SCALE
  %t95 = sdiv i64 %t96, %t99
  %t91 = add nsw i64 %t94, %t95
  %t101 = mul nsw i64 %t70, %t70
  %t104 = load i64, ptr @SCALE
  %t100 = sdiv i64 %t101, %t104
  %t90 = add nsw i64 %t91, %t100
  %t108 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t109 = load i64, ptr %t108, align 8
  %t110 = add i64 0, 5000000
  %t106 = srem i64 %t109, %t110
  %t111 = add i64 0, 0
  %t112 = icmp eq i64 %t106, %t111
  %t105 = zext i1 %t112 to i8
  %t114 = trunc i8 %t105 to i1
  br i1 %t114, label %guard.then113, label %guard.end113
  guard.then113:
   %t116 = call i64 @__print_int(i64 %t90)
  %t119 = add i64 0, 10
   %t118 = call i64 @__print_char(i64 %t119)
  br label %guard.end113
  guard.end113:
  %t122 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store i64 %t90, ptr %t122
  %t125 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t126 = load i64, ptr %t125, align 8
  %t127 = add i64 0, 1
  %t123 = add nsw i64 %t126, %t127
  %t128 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t123, ptr %t128
  %t131 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t132 = load i64, ptr %t131, align 8
  %t134 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t135 = load i64, ptr %t134, align 8
  %t136 = icmp eq i64 %t132, %t135
  %t129 = zext i1 %t136 to i8
  %t138 = trunc i8 %t129 to i1
  br i1 %t138, label %guard.then137, label %guard.end137
  guard.then137:
   %t140 = call i64 @__print_int(i64 %t90)
  %t143 = add i64 0, 10
   %t142 = call i64 @__print_char(i64 %t143)
  br label %guard.end137
  guard.end137:
  ret void
}

define internal i8 @pre_mb(ptr noundef noalias nocapture align 8 %state) #10 {
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
  store i64 100, ptr %ip_3, align 8
  %ip_4 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 0, ptr %ip_4, align 8
  %ip_5 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store i64 -75, ptr %ip_5, align 8
  %ip_6 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 10, ptr %ip_6, align 8
  %ip_7 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store i64 0, ptr %ip_7, align 8
  %ip_8 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store i64 0, ptr %ip_8, align 8
  %ip_9 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store i64 0, ptr %ip_9, align 8
  %ip_10 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store i64 0, ptr %ip_10, align 8
  %ip_11 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  store i64 0, ptr %ip_11, align 8
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
  store i64 100, ptr %t13, align 8
  %t14 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 0, ptr %t14, align 8
  %t15 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store i64 -75, ptr %t15, align 8
  %t16 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 10, ptr %t16, align 8
  %t17 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store i64 0, ptr %t17, align 8
  %t18 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store i64 0, ptr %t18, align 8
  %t19 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store i64 0, ptr %t19, align 8
  %t20 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store i64 0, ptr %t20, align 8
  %t21 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  store i64 0, ptr %t21, align 8
  %clb23 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %whb22 = load i64, ptr %clb23, align 8
  br label %.wloop
.wloop:
  %t24 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t25 = load i64, ptr %t24, align 8
  %whd26 = icmp slt i64 %t25, %whb22
  br i1 %whd26, label %.wbody, label %.wend
.wbody:
  %t31 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t32 = load i64, ptr %t31, align 8
  %t33 = load i64, ptr @IA
  %t29 = mul nsw i64 %t32, %t33
  %t34 = load i64, ptr @IC
  %t28 = add nsw i64 %t29, %t34
  %t35 = load i64, ptr @IM
  %t27 = srem i64 %t28, %t35
  %t39 = add i64 0, 200
  %t37 = srem i64 %t27, %t39
  %t40 = add i64 0, 100
  %t36 = sub nsw i64 %t37, %t40
  %t45 = load i64, ptr @IA
  %t43 = mul nsw i64 %t27, %t45
  %t46 = load i64, ptr @IC
  %t42 = add nsw i64 %t43, %t46
  %t47 = load i64, ptr @IM
  %t41 = srem i64 %t42, %t47
  %t51 = add i64 0, 200
  %t49 = srem i64 %t41, %t51
  %t52 = add i64 0, 100
  %t48 = sub nsw i64 %t49, %t52
  %t56 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t57 = load i64, ptr %t56, align 8
  %t54 = mul nsw i64 %t57, %t36
  %t59 = load i64, ptr @SCALE
  %t53 = sdiv i64 %t54, %t59
  %t63 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t64 = load i64, ptr %t63, align 8
  %t61 = mul nsw i64 %t64, %t48
  %t66 = load i64, ptr @SCALE
  %t60 = sdiv i64 %t61, %t66
  %t70 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t71 = load i64, ptr %t70, align 8
  %t68 = mul nsw i64 %t71, %t48
  %t73 = load i64, ptr @SCALE
  %t67 = sdiv i64 %t68, %t73
  %t74 = sub nsw i64 %t53, %t60
  %t82 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t83 = load i64, ptr %t82, align 8
  %t80 = mul nsw i64 %t83, %t36
  %t85 = load i64, ptr @SCALE
  %t79 = sdiv i64 %t80, %t85
  %t77 = add nsw i64 %t67, %t79
  %t89 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t90 = load i64, ptr %t89, align 8
  %t92 = mul nsw i64 %t74, %t74
  %t95 = load i64, ptr @SCALE
  %t91 = sdiv i64 %t92, %t95
  %t87 = add nsw i64 %t90, %t91
  %t97 = mul nsw i64 %t77, %t77
  %t100 = load i64, ptr @SCALE
  %t96 = sdiv i64 %t97, %t100
  %t86 = add nsw i64 %t87, %t96
  %cms102 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 %t41, ptr %cms102, align 8
  %cms104 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store i64 %t36, ptr %cms104, align 8
  %cms106 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 %t48, ptr %cms106, align 8
  %cms108 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store i64 %t53, ptr %cms108, align 8
  %cms110 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store i64 %t60, ptr %cms110, align 8
  %cms112 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store i64 %t67, ptr %cms112, align 8
  %cms114 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 %t74, ptr %cms114, align 8
  %cms116 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 %t77, ptr %cms116, align 8
  %cms118 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store i64 %t86, ptr %cms118, align 8
  %t122 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t123 = load i64, ptr %t122, align 8
  %t124 = add i64 0, 5000000
  %t120 = srem i64 %t123, %t124
  %t125 = add i64 0, 0
  %t126 = icmp eq i64 %t120, %t125
  %t119 = zext i1 %t126 to i8
  %tb127 = trunc i8 %t119 to i1
  br i1 %tb127, label %.cmgb128, label %.cmgn128
.cmgb128:
   %t130 = call i64 @__print_int(i64 %t86)
  %t133 = add i64 0, 10
   %t132 = call i64 @__print_char(i64 %t133)
  br label %.cmgn128
.cmgn128:
  %t136 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t137 = load i64, ptr %t136, align 8
  %t138 = add i64 0, 1
  %t134 = add nsw i64 %t137, %t138
  %cms139 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t134, ptr %cms139, align 8
  %whn140 = add nuw nsw i64 %t25, 1
  %t141 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %whn140, ptr %t141, align 8
  br label %.wloop, !llvm.loop !100
  br label %.wloop
.wend:
  %t145 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t146 = load i64, ptr %t145, align 8
   %t143 = call i64 @__print_int(i64 %t146)
  %t148 = add i64 0, 10
   %t147 = call i64 @__print_char(i64 %t148)
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
