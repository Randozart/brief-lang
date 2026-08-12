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
declare i64 @__print_char(i64) #6
declare { i64, i64 } @__getenv_briev({ i64, i64 }) #6
declare void @__print_str({ i64, i64 }) #6
declare i64 @__print_int(i64) #6
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
@A02 = constant float bitcast (i32 0 to float)
@A10 = alias float, float* @A02
@A11 = alias float, float* @A00
@A12 = alias float, float* @A01
@A20 = alias float, float* @A02
@A21 = alias float, float* @A02
@A22 = alias float, float* @A00
@Q00 = constant float bitcast (i32 1036831949 to float)
@Q01 = alias float, float* @A02
@Q02 = alias float, float* @A02
@Q10 = alias float, float* @A02
@Q11 = alias float, float* @Q00
@Q12 = alias float, float* @A02
@Q20 = alias float, float* @A02
@Q21 = alias float, float* @A02
@Q22 = alias float, float* @Q00

%StateChunk0 = type { float, float, float, float, float, float, float, float, float, float, float, float, i64, i64, i64 }
%State = type { float, float, float, float, float, float, float, float, float, float, float, float, i64, i64, i64 }
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

define void @txn_tick(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #11 alwaysinline {
  entry:
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t7 = load i64, ptr %t6, align 8
  %t9 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
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
  %t32 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store float %t15, ptr %t32
  %t35 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t36 = load float, ptr %t35, align 4
  %t37 = load float, ptr @Q00
  %t33 = fadd fast float %t36, %t37
  %t38 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store float %t33, ptr %t38
  %t41 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t42 = load float, ptr %t41, align 4
  %t43 = load float, ptr @Q01
  %t39 = fadd fast float %t42, %t43
  %t44 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store float %t39, ptr %t44
  %t47 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t48 = load float, ptr %t47, align 4
  %t49 = load float, ptr @Q02
  %t45 = fadd fast float %t48, %t49
  %t50 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store float %t45, ptr %t50
  %t53 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t54 = load float, ptr %t53, align 4
  %t55 = load float, ptr @Q10
  %t51 = fadd fast float %t54, %t55
  %t56 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store float %t51, ptr %t56
  %t59 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t60 = load float, ptr %t59, align 4
  %t61 = load float, ptr @Q11
  %t57 = fadd fast float %t60, %t61
  %t62 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store float %t57, ptr %t62
  %t65 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t66 = load float, ptr %t65, align 4
  %t67 = load float, ptr @Q12
  %t63 = fadd fast float %t66, %t67
  %t68 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store float %t63, ptr %t68
  %t71 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t72 = load float, ptr %t71, align 4
  %t73 = load float, ptr @Q20
  %t69 = fadd fast float %t72, %t73
  %t74 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store float %t69, ptr %t74
  %t77 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t78 = load float, ptr %t77, align 4
  %t79 = load float, ptr @Q21
  %t75 = fadd fast float %t78, %t79
  %t80 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store float %t75, ptr %t80
  %t83 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t84 = load float, ptr %t83, align 4
  %t85 = load float, ptr @Q22
  %t81 = fadd fast float %t84, %t85
  %t86 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  store float %t81, ptr %t86
  %t89 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t90 = load i64, ptr %t89, align 8
  %t91 = add i64 0, 1
  %t87 = add nsw i64 %t90, %t91
  %t92 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store i64 %t87, ptr %t92
  %t96 = load float, ptr @A10
  %t98 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t99 = load float, ptr %t98, align 4
  %t95 = fmul fast float %t96, %t99
  %t101 = load float, ptr @A11
  %t103 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t104 = load float, ptr %t103, align 4
  %t100 = fmul fast float %t101, %t104
  %t94 = fadd fast float %t95, %t100
  %t106 = load float, ptr @A12
  %t108 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t109 = load float, ptr %t108, align 4
  %t105 = fmul fast float %t106, %t109
  %t93 = fadd fast float %t94, %t105
  %t110 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store float %t93, ptr %t110
  %t114 = load float, ptr @A20
  %t116 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t117 = load float, ptr %t116, align 4
  %t113 = fmul fast float %t114, %t117
  %t119 = load float, ptr @A21
  %t121 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t122 = load float, ptr %t121, align 4
  %t118 = fmul fast float %t119, %t122
  %t112 = fadd fast float %t113, %t118
  %t124 = load float, ptr @A22
  %t126 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t127 = load float, ptr %t126, align 4
  %t123 = fmul fast float %t124, %t127
  %t111 = fadd fast float %t112, %t123
  %t128 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store float %t111, ptr %t128
  %t132 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t133 = load i64, ptr %t132, align 8
  %t134 = add i64 0, 5000000
  %t130 = srem i64 %t133, %t134
  %t135 = add i64 0, 0
  %t136 = icmp eq i64 %t130, %t135
  %t129 = zext i1 %t136 to i8
  %t138 = trunc i8 %t129 to i1
  br i1 %t138, label %guard.then137, label %guard.end137
  guard.then137:
    %t139 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
    %t140 = load float, ptr %t139
    %t141 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
    %t142 = load float, ptr %t141
    %t143 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
    %t144 = load float, ptr %t143
    %t145 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
    %t146 = load float, ptr %t145
    %t147 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
    %t148 = load float, ptr %t147
    %t149 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
    %t150 = load float, ptr %t149
    %t151 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
    %t152 = load float, ptr %t151
    %t153 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
    %t154 = load float, ptr %t153
    %t155 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
    %t156 = load float, ptr %t155
    %t157 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
    %t158 = load float, ptr %t157
    %t159 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
    %t160 = load float, ptr %t159
    %t161 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
    %t162 = load float, ptr %t161
    call void @txn_tick_cold_0(float %t140, float %t142, float %t144, float %t146, float %t148, float %t150, float %t152, float %t154, float %t156, float %t158, float %t160, float %t162)
    br label %guard.end137
  guard.end137:
  ret void
}
define void @txn_tick_cold_0(float %__cp_p00, float %__cp_p01, float %__cp_p02, float %__cp_p10, float %__cp_p11, float %__cp_p12, float %__cp_p20, float %__cp_p21, float %__cp_p22, float %__cp_x0, float %__cp_x1, float %__cp_x2) local_unnamed_addr #0 {
  %t171 = fadd fast float %__cp_p00, %__cp_p01
  %t170 = fadd fast float %t171, %__cp_p02
  %t169 = fadd fast float %t170, %__cp_p10
  %t168 = fadd fast float %t169, %__cp_p11
  %t167 = fadd fast float %t168, %__cp_p12
  %t166 = fadd fast float %t167, %__cp_p20
  %t165 = fadd fast float %t166, %__cp_p21
  %t164 = fadd fast float %t165, %__cp_p22
  %t185 = fadd fast float %__cp_x0, %__cp_x1
  %t184 = fadd fast float %t185, %__cp_x2
  %t183 = fadd fast float %t184, %t164
   %t182 = call i64 @__print_float(float %t183)
  %t191 = add i64 0, 10
   %t190 = call i64 @__print_char(i64 %t191)
  ret void
}


define internal i8 @pre_tick(ptr noundef noalias nocapture align 8 %state) #10 {
  entry:
  %t2 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t3 = load i64, ptr %t2, align 8
  %t5 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t6 = load i64, ptr %t5, align 8
  %t7 = icmp slt i64 %t3, %t6
  %t0 = zext i1 %t7 to i8
  ret i8 %t0
}
define void @init_state(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #0 {
  entry:
  %ip_0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %ip_0b = bitcast i32 0 to float
  store float %ip_0b, ptr %ip_0, align 4
  %ip_1 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %ip_1b = bitcast i32 0 to float
  store float %ip_1b, ptr %ip_1, align 4
  %ip_2 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %ip_2b = bitcast i32 0 to float
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
  %ip_6b = bitcast i32 0 to float
  store float %ip_6b, ptr %ip_6, align 4
  %ip_7 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %ip_7b = bitcast i32 0 to float
  store float %ip_7b, ptr %ip_7, align 4
  %ip_8 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %ip_8b = bitcast i32 0 to float
  store float %ip_8b, ptr %ip_8, align 4
  %ip_9 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %ip_9b = bitcast i32 0 to float
  store float %ip_9b, ptr %ip_9, align 4
  %ip_10 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %ip_10b = bitcast i32 0 to float
  store float %ip_10b, ptr %ip_10, align 4
  %ip_11 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %ip_11b = bitcast i32 0 to float
  store float %ip_11b, ptr %ip_11, align 4
  %ip_12 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store i64 0, ptr %ip_12, align 8
  %ip_13 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t2 = bitcast <{ i64, [6 x i8] }>* @str.1 to ptr
  %t3 = ptrtoint ptr %t2 to i64
  %t4 = inttoptr i64 %t3 to ptr
  %t0 = call i64 @get_env_int(ptr %state, ptr %t4)
  store i64 %t0, ptr %ip_13, align 8
  %ip_14 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  store i64 0, ptr %ip_14, align 8
  ret void
}


define i32 @main() local_unnamed_addr #9 {
entry:
  %state = alloca %State, align 8
  %t5 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %ip_0b = bitcast i32 0 to float
  store float %ip_0b, ptr %t5, align 4
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %ip_1b = bitcast i32 0 to float
  store float %ip_1b, ptr %t6, align 4
  %t7 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %ip_2b = bitcast i32 0 to float
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
  %ip_6b = bitcast i32 0 to float
  store float %ip_6b, ptr %t11, align 4
  %t12 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %ip_7b = bitcast i32 0 to float
  store float %ip_7b, ptr %t12, align 4
  %t13 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %ip_8b = bitcast i32 0 to float
  store float %ip_8b, ptr %t13, align 4
  %t14 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %ip_9b = bitcast i32 0 to float
  store float %ip_9b, ptr %t14, align 4
  %t15 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %ip_10b = bitcast i32 0 to float
  store float %ip_10b, ptr %t15, align 4
  %t16 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %ip_11b = bitcast i32 0 to float
  store float %ip_11b, ptr %t16, align 4
  %t17 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store i64 0, ptr %t17, align 8
  %t18 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t21 = bitcast <{ i64, [6 x i8] }>* @str.1 to ptr
  %t22 = ptrtoint ptr %t21 to i64
  %t23 = inttoptr i64 %t22 to ptr
  %t19 = call i64 @get_env_int(ptr %state, ptr %t23)
  store i64 %t19, ptr %t18, align 8
  %t24 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  store i64 0, ptr %t24, align 8
  %clb26 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %whb25 = load i64, ptr %clb26, align 8
  br label %.wloop
.wloop:
  %t27 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t28 = load i64, ptr %t27, align 8
  %whd29 = icmp slt i64 %t28, %whb25
  br i1 %whd29, label %.wbody, label %.wend
.wbody:
  %t33 = load float, ptr @A00
  %t35 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t36 = load float, ptr %t35, align 4
  %t32 = fmul fast float %t33, %t36
  %t38 = load float, ptr @A01
  %t40 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t41 = load float, ptr %t40, align 4
  %t37 = fmul fast float %t38, %t41
  %t31 = fadd fast float %t32, %t37
  %t43 = load float, ptr @A02
  %t45 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t46 = load float, ptr %t45, align 4
  %t42 = fmul fast float %t43, %t46
  %t30 = fadd fast float %t31, %t42
  %cms47 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store float %t30, ptr %cms47, align 8
  %t51 = load float, ptr @A10
  %t50 = fmul fast float %t51, %t30
  %t54 = load float, ptr @A11
  %t56 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t57 = load float, ptr %t56, align 4
  %t53 = fmul fast float %t54, %t57
  %t49 = fadd fast float %t50, %t53
  %t59 = load float, ptr @A12
  %t61 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t62 = load float, ptr %t61, align 4
  %t58 = fmul fast float %t59, %t62
  %t48 = fadd fast float %t49, %t58
  %cms63 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store float %t48, ptr %cms63, align 8
  %t67 = load float, ptr @A20
  %t66 = fmul fast float %t67, %t30
  %t70 = load float, ptr @A21
  %t69 = fmul fast float %t70, %t48
  %t65 = fadd fast float %t66, %t69
  %t73 = load float, ptr @A22
  %t75 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t76 = load float, ptr %t75, align 4
  %t72 = fmul fast float %t73, %t76
  %t64 = fadd fast float %t65, %t72
  %cms77 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store float %t64, ptr %cms77, align 8
  %t80 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t81 = load float, ptr %t80, align 4
  %t82 = load float, ptr @Q00
  %t78 = fadd fast float %t81, %t82
  %cms83 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store float %t78, ptr %cms83, align 8
  %t86 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t87 = load float, ptr %t86, align 4
  %t88 = load float, ptr @Q01
  %t84 = fadd fast float %t87, %t88
  %cms89 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store float %t84, ptr %cms89, align 8
  %t92 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t93 = load float, ptr %t92, align 4
  %t94 = load float, ptr @Q02
  %t90 = fadd fast float %t93, %t94
  %cms95 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store float %t90, ptr %cms95, align 8
  %t98 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t99 = load float, ptr %t98, align 4
  %t100 = load float, ptr @Q10
  %t96 = fadd fast float %t99, %t100
  %cms101 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store float %t96, ptr %cms101, align 8
  %t104 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t105 = load float, ptr %t104, align 4
  %t106 = load float, ptr @Q11
  %t102 = fadd fast float %t105, %t106
  %cms107 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store float %t102, ptr %cms107, align 8
  %t110 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t111 = load float, ptr %t110, align 4
  %t112 = load float, ptr @Q12
  %t108 = fadd fast float %t111, %t112
  %cms113 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store float %t108, ptr %cms113, align 8
  %t116 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t117 = load float, ptr %t116, align 4
  %t118 = load float, ptr @Q20
  %t114 = fadd fast float %t117, %t118
  %cms119 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store float %t114, ptr %cms119, align 8
  %t122 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t123 = load float, ptr %t122, align 4
  %t124 = load float, ptr @Q21
  %t120 = fadd fast float %t123, %t124
  %cms125 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store float %t120, ptr %cms125, align 8
  %t128 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t129 = load float, ptr %t128, align 4
  %t130 = load float, ptr @Q22
  %t126 = fadd fast float %t129, %t130
  %cms131 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  store float %t126, ptr %cms131, align 8
  %t134 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t135 = load i64, ptr %t134, align 8
  %t136 = add i64 0, 1
  %t132 = add nsw i64 %t135, %t136
  %cms137 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store i64 %t132, ptr %cms137, align 8
  %t141 = add i64 0, 5000000
  %t139 = srem i64 %t132, %t141
  %t142 = add i64 0, 0
  %t143 = icmp eq i64 %t139, %t142
  %t138 = zext i1 %t143 to i8
  %tb144 = trunc i8 %t138 to i1
  br i1 %tb144, label %.cmgb145, label %.cmgn145
.cmgb145:
  %t153 = fadd fast float %t78, %t84
  %t152 = fadd fast float %t153, %t90
  %t151 = fadd fast float %t152, %t96
  %t150 = fadd fast float %t151, %t102
  %t149 = fadd fast float %t150, %t108
  %t148 = fadd fast float %t149, %t114
  %t147 = fadd fast float %t148, %t120
  %t146 = fadd fast float %t147, %t126
  %t167 = fadd fast float %t30, %t48
  %t166 = fadd fast float %t167, %t64
  %t165 = fadd fast float %t166, %t146
   %t164 = call i64 @__print_float(float %t165)
  %t173 = add i64 0, 10
   %t172 = call i64 @__print_char(i64 %t173)
  br label %.cmgn145
.cmgn145:
  %whn174 = add nuw nsw i64 %t28, 1
  %t175 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store i64 %whn174, ptr %t175, align 8
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
