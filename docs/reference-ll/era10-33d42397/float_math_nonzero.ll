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
declare i64 @__print_int(i64) #6
declare { i64, i64 } @__getenv_briev({ i64, i64 }) #6
declare void @__print_str({ i64, i64 }) #6
declare i64 @__print_char(i64) #6
declare i64 @__print_float(float) #6
declare i64 @__getenv_int({ i64, i64 }) #6
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
@A00 = constant float bitcast (i32 1065353216 to float)
@A01 = constant float bitcast (i32 1008981770 to float)
@A02 = constant float bitcast (i32 981668463 to float)
@A10 = alias float, float* @A01
@A11 = alias float, float* @A00
@A12 = alias float, float* @A01
@A20 = alias float, float* @A02
@A21 = alias float, float* @A01
@A22 = alias float, float* @A00
@Q00 = constant float bitcast (i32 1036831949 to float)
@Q11 = alias float, float* @Q00
@Q22 = alias float, float* @Q00

%StateChunk0 = type { float, float, float, float, float, float, i64, i64, i64 }
%State = type { float, float, float, float, float, float, i64, i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant <{ i64, [1 x i8] }> <{ i64 0, [1 x i8] c"\00" }>, align 8
@str.1 = private unnamed_addr constant <{ i64, [6 x i8] }> <{ i64 5, [6 x i8] c"BOUND\00" }>, align 8

@ll_empty_list = private unnamed_addr constant { i64, i64 } { i64 0, i64 0 }

define ptr @get_env(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
    %t2 = inttoptr i64 %ac0 to ptr
   %t0 = call ptr @__getenv_briev(ptr %t2)
  ret ptr %t0
}

define i64 @get_env_int(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
    %t2 = inttoptr i64 %ac0 to ptr
   %t0 = call i64 @__getenv_int(ptr %t2)
  ret i64 %t0
}

define void @txn_tick(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t7 = load i64, ptr %t6, align 8
  %t9 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t10 = load i64, ptr %t9, align 8
  %t11 = icmp slt i64 %t7, %t10
  %t4 = zext i1 %t11 to i8
  %pi12 = trunc i8 %t4 to i1
  br i1 %pi12, label %ps14, label %pp13
  pp13:
    unreachable
  ps14:
  call void @llvm.assume(i1 %pi12)
  %t18 = load float, ptr @A00
  %t20 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t21 = load float, ptr %t20, align 4
  %t17 = fmul fast float %t18, %t21
  %t23 = load float, ptr @A01
  %t25 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t26 = load float, ptr %t25, align 4
  %t22 = fmul fast float %t23, %t26
  %t16 = fadd fast float %t17, %t22
  %t28 = load float, ptr @A02
  %t30 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t31 = load float, ptr %t30, align 4
  %t27 = fmul fast float %t28, %t31
  %t15 = fadd fast float %t16, %t27
  %t35 = load float, ptr @A10
  %t37 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t38 = load float, ptr %t37, align 4
  %t34 = fmul fast float %t35, %t38
  %t40 = load float, ptr @A11
  %t42 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t43 = load float, ptr %t42, align 4
  %t39 = fmul fast float %t40, %t43
  %t33 = fadd fast float %t34, %t39
  %t45 = load float, ptr @A12
  %t47 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t48 = load float, ptr %t47, align 4
  %t44 = fmul fast float %t45, %t48
  %t32 = fadd fast float %t33, %t44
  %t52 = load float, ptr @A20
  %t54 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t55 = load float, ptr %t54, align 4
  %t51 = fmul fast float %t52, %t55
  %t57 = load float, ptr @A21
  %t59 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t60 = load float, ptr %t59, align 4
  %t56 = fmul fast float %t57, %t60
  %t50 = fadd fast float %t51, %t56
  %t62 = load float, ptr @A22
  %t64 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t65 = load float, ptr %t64, align 4
  %t61 = fmul fast float %t62, %t65
  %t49 = fadd fast float %t50, %t61
  %t68 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t69 = load float, ptr %t68, align 4
  %t70 = load float, ptr @Q00
  %t66 = fadd fast float %t69, %t70
  %t71 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store float %t66, ptr %t71
  %t74 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t75 = load float, ptr %t74, align 4
  %t76 = load float, ptr @Q11
  %t72 = fadd fast float %t75, %t76
  %t77 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store float %t72, ptr %t77
  %t80 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t81 = load float, ptr %t80, align 4
  %t82 = load float, ptr @Q22
  %t78 = fadd fast float %t81, %t82
  %t83 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store float %t78, ptr %t83
  %t86 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t87 = load i64, ptr %t86, align 8
  %t88 = add i64 0, 1
  %t84 = add nsw i64 %t87, %t88
  %t89 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 %t84, ptr %t89
  %t91 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store float %t32, ptr %t91
  %t93 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store float %t15, ptr %t93
  %t95 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store float %t49, ptr %t95
  %t99 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t100 = load i64, ptr %t99, align 8
  %t101 = add i64 0, 5000000
  %t97 = srem i64 %t100, %t101
  %t102 = add i64 0, 0
  %t103 = icmp eq i64 %t97, %t102
  %t96 = zext i1 %t103 to i8
  %t105 = trunc i8 %t96 to i1
  br i1 %t105, label %guard.then104, label %guard.end104
  guard.then104:
  %t109 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t110 = load float, ptr %t109, align 4
  %t112 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t113 = load float, ptr %t112, align 4
  %t107 = fadd fast float %t110, %t113
  %t115 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t116 = load float, ptr %t115, align 4
  %t106 = fadd fast float %t107, %t116
  %t122 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t123 = load float, ptr %t122, align 4
  %t125 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t126 = load float, ptr %t125, align 4
  %t120 = fadd fast float %t123, %t126
  %t128 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t129 = load float, ptr %t128, align 4
  %t119 = fadd fast float %t120, %t129
  %t118 = fadd fast float %t119, %t106
   %t117 = call i64 @__print_float(float %t118)
  %t132 = add i64 0, 10
   %t131 = call i64 @__print_char(i64 %t132)
  br label %guard.end104
  guard.end104:
  ret void
}

define internal i8 @pre_tick(ptr noalias nocapture align 8 %state) #10 {
  entry:
  %t2 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t3 = load i64, ptr %t2, align 8
  %t5 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t6 = load i64, ptr %t5, align 8
  %t7 = icmp slt i64 %t3, %t6
  %t0 = zext i1 %t7 to i8
  ret i8 %t0
}
define void @init_state(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 {
  entry:
  %ip_0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %ip_0b = bitcast i32 1065353216 to float
  store float %ip_0b, ptr %ip_0, align 4
  %ip_1 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %ip_1b = bitcast i32 1056964608 to float
  store float %ip_1b, ptr %ip_1, align 4
  %ip_2 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %ip_2b = bitcast i32 1045220557 to float
  store float %ip_2b, ptr %ip_2, align 4
  %ip_3 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %ip_3b = bitcast i32 0 to float
  store float %ip_3b, ptr %ip_3, align 4
  %ip_4 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %ip_4b = bitcast i32 0 to float
  store float %ip_4b, ptr %ip_4, align 4
  %ip_5 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %ip_5b = bitcast i32 0 to float
  store float %ip_5b, ptr %ip_5, align 4
  %ip_6 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 0, ptr %ip_6, align 8
  %ip_7 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t2 = bitcast <{ i64, [6 x i8] }>* @str.1 to ptr
  %t3 = ptrtoint ptr %t2 to i64
  %t4 = inttoptr i64 %t3 to ptr
  %t0 = call i64 @get_env_int(ptr %state, ptr %t4)
  store i64 %t0, ptr %ip_7, align 8
  %ip_8 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store i64 0, ptr %ip_8, align 8
  ret void
}


define i32 @main() local_unnamed_addr #9 {
entry:
  %state = alloca %State, align 8
  %t5 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %ip_0b = bitcast i32 1065353216 to float
  store float %ip_0b, ptr %t5, align 4
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %ip_1b = bitcast i32 1056964608 to float
  store float %ip_1b, ptr %t6, align 4
  %t7 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %ip_2b = bitcast i32 1045220557 to float
  store float %ip_2b, ptr %t7, align 4
  %t8 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %ip_3b = bitcast i32 0 to float
  store float %ip_3b, ptr %t8, align 4
  %t9 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %ip_4b = bitcast i32 0 to float
  store float %ip_4b, ptr %t9, align 4
  %t10 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %ip_5b = bitcast i32 0 to float
  store float %ip_5b, ptr %t10, align 4
  %t11 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 0, ptr %t11, align 8
  %t12 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t15 = bitcast <{ i64, [6 x i8] }>* @str.1 to ptr
  %t16 = ptrtoint ptr %t15 to i64
  %t17 = inttoptr i64 %t16 to ptr
  %t13 = call i64 @get_env_int(ptr %state, ptr %t17)
  store i64 %t13, ptr %t12, align 8
  %t18 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store i64 0, ptr %t18, align 8
  %clb20 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %whb19 = load i64, ptr %clb20, align 8
  br label %.wloop
.wloop:
  %t21 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t22 = load i64, ptr %t21, align 8
  %whd23 = icmp slt i64 %t22, %whb19
  br i1 %whd23, label %.wbody, label %.wend
.wbody:
  %t27 = load float, ptr @A00
  %t29 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t30 = load float, ptr %t29, align 4
  %t26 = fmul fast float %t27, %t30
  %t32 = load float, ptr @A01
  %t34 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t35 = load float, ptr %t34, align 4
  %t31 = fmul fast float %t32, %t35
  %t25 = fadd fast float %t26, %t31
  %t37 = load float, ptr @A02
  %t39 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t40 = load float, ptr %t39, align 4
  %t36 = fmul fast float %t37, %t40
  %t24 = fadd fast float %t25, %t36
  %t44 = load float, ptr @A10
  %t46 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t47 = load float, ptr %t46, align 4
  %t43 = fmul fast float %t44, %t47
  %t49 = load float, ptr @A11
  %t51 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t52 = load float, ptr %t51, align 4
  %t48 = fmul fast float %t49, %t52
  %t42 = fadd fast float %t43, %t48
  %t54 = load float, ptr @A12
  %t56 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t57 = load float, ptr %t56, align 4
  %t53 = fmul fast float %t54, %t57
  %t41 = fadd fast float %t42, %t53
  %t61 = load float, ptr @A20
  %t63 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t64 = load float, ptr %t63, align 4
  %t60 = fmul fast float %t61, %t64
  %t66 = load float, ptr @A21
  %t68 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t69 = load float, ptr %t68, align 4
  %t65 = fmul fast float %t66, %t69
  %t59 = fadd fast float %t60, %t65
  %t71 = load float, ptr @A22
  %t73 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t74 = load float, ptr %t73, align 4
  %t70 = fmul fast float %t71, %t74
  %t58 = fadd fast float %t59, %t70
  %cms76 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store float %t24, ptr %cms76, align 8
  %cms78 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store float %t41, ptr %cms78, align 8
  %cms80 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store float %t58, ptr %cms80, align 8
  %t83 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t84 = load float, ptr %t83, align 4
  %t85 = load float, ptr @Q00
  %t81 = fadd fast float %t84, %t85
  %cms86 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store float %t81, ptr %cms86, align 8
  %t89 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t90 = load float, ptr %t89, align 4
  %t91 = load float, ptr @Q11
  %t87 = fadd fast float %t90, %t91
  %cms92 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store float %t87, ptr %cms92, align 8
  %t95 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t96 = load float, ptr %t95, align 4
  %t97 = load float, ptr @Q22
  %t93 = fadd fast float %t96, %t97
  %cms98 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store float %t93, ptr %cms98, align 8
  %t101 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t102 = load i64, ptr %t101, align 8
  %t103 = add i64 0, 1
  %t99 = add nsw i64 %t102, %t103
  %cms104 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 %t99, ptr %cms104, align 8
  %t108 = add i64 0, 5000000
  %t106 = srem i64 %t99, %t108
  %t109 = add i64 0, 0
  %t110 = icmp eq i64 %t106, %t109
  %t105 = zext i1 %t110 to i8
  %tb111 = trunc i8 %t105 to i1
  br i1 %tb111, label %.cmgb112, label %.cmgn112
.cmgb112:
  %t114 = fadd fast float %t81, %t87
  %t113 = fadd fast float %t114, %t93
  %t121 = fadd fast float %t24, %t41
  %t120 = fadd fast float %t121, %t58
  %t119 = fadd fast float %t120, %t113
   %t118 = call i64 @__print_float(float %t119)
  %t127 = add i64 0, 10
   %t126 = call i64 @__print_char(i64 %t127)
  br label %.cmgn112
.cmgn112:
  %whn128 = add nuw nsw i64 %t22, 1
  %t129 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 %whn128, ptr %t129, align 8
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
    nofree norecurse nosync nounwind memory(argmem: readwrite)
}
attributes #10 = {
    mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: read)
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
