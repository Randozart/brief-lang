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
declare i64 @__getenv_int({ i64, i64 }) #6
declare { i64, i64 } @__getenv_brief({ i64, i64 }) #6
declare void @__print_str({ i64, i64 }) #6
declare i64 @__print_int(i64) #6
declare i64 @__print_char(i64) #6
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

define void @txn_ping(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #11 alwaysinline {
  entry:
  %t7 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t8 = load i64, ptr %t7, align 8
  %t10 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t11 = load i64, ptr %t10, align 8
  %t12 = icmp slt i64 %t8, %t11
  %t5 = zext i1 %t12 to i8
  %t16 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t17 = load i64, ptr %t16, align 8
  %t18 = add i64 0, 8
  %t14 = srem i64 %t17, %t18
  %t19 = add i64 0, 0
  %t20 = icmp eq i64 %t14, %t19
  %t13 = zext i1 %t20 to i8
  %t4 = and i8 %t5, %t13
  %pi21 = trunc i8 %t4 to i1
  br i1 %pi21, label %ps23, label %pp22
  pp22:
    unreachable
  ps23:
  call void @llvm.assume(i1 %pi21)
  %t26 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t27 = load i64, ptr %t26, align 8
  %t28 = add i64 0, 1
  %t24 = add nsw i64 %t27, %t28
  %t29 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t24, ptr %t29
  %t33 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t34 = load i64, ptr %t33, align 8
  %t35 = add i64 0, 5000000
  %t31 = srem i64 %t34, %t35
  %t36 = add i64 0, 4999999
  %t37 = icmp eq i64 %t31, %t36
  %t30 = zext i1 %t37 to i8
  %t39 = trunc i8 %t30 to i1
  br i1 %t39, label %guard.then38, label %guard.end38
  guard.then38:
    %t40 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
    %t41 = load i64, ptr %t40
    call void @txn_ping_cold_0(i64 %t41)
    br label %guard.end38
  guard.end38:
  ret void
}
define void @txn_ping_cold_0(i64 %__cp_count) local_unnamed_addr #0 {
  %t47 = add i64 0, 1
  %t45 = add nsw i64 %__cp_count, %t47
   %t44 = call i64 @__print_int(i64 %t45)
  %t49 = add i64 0, 10
   %t48 = call i64 @__print_char(i64 %t49)
  ret void
}


define void @txn_ack(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #11 alwaysinline {
  entry:
  %t53 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t54 = load i64, ptr %t53, align 8
  %t56 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t57 = load i64, ptr %t56, align 8
  %t58 = icmp slt i64 %t54, %t57
  %t51 = zext i1 %t58 to i8
  %t62 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t63 = load i64, ptr %t62, align 8
  %t64 = add i64 0, 8
  %t60 = srem i64 %t63, %t64
  %t65 = add i64 0, 1
  %t66 = icmp eq i64 %t60, %t65
  %t59 = zext i1 %t66 to i8
  %t50 = and i8 %t51, %t59
  %pi67 = trunc i8 %t50 to i1
  br i1 %pi67, label %ps69, label %pp68
  pp68:
    unreachable
  ps69:
  call void @llvm.assume(i1 %pi67)
  %t72 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t73 = load i64, ptr %t72, align 8
  %t74 = add i64 0, 1
  %t70 = add nsw i64 %t73, %t74
  %t75 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t70, ptr %t75
  %t79 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t80 = load i64, ptr %t79, align 8
  %t81 = add i64 0, 5000000
  %t77 = srem i64 %t80, %t81
  %t82 = add i64 0, 4999999
  %t83 = icmp eq i64 %t77, %t82
  %t76 = zext i1 %t83 to i8
  %t85 = trunc i8 %t76 to i1
  br i1 %t85, label %guard.then84, label %guard.end84
  guard.then84:
    %t86 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
    %t87 = load i64, ptr %t86
    call void @txn_ack_cold_0(i64 %t87)
    br label %guard.end84
  guard.end84:
  ret void
}
define void @txn_ack_cold_0(i64 %__cp_count) local_unnamed_addr #0 {
  %t93 = add i64 0, 1
  %t91 = add nsw i64 %__cp_count, %t93
   %t90 = call i64 @__print_int(i64 %t91)
  %t95 = add i64 0, 10
   %t94 = call i64 @__print_char(i64 %t95)
  ret void
}


define void @txn_err(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #11 alwaysinline {
  entry:
  %t99 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t100 = load i64, ptr %t99, align 8
  %t102 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t103 = load i64, ptr %t102, align 8
  %t104 = icmp slt i64 %t100, %t103
  %t97 = zext i1 %t104 to i8
  %t108 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t109 = load i64, ptr %t108, align 8
  %t110 = add i64 0, 8
  %t106 = srem i64 %t109, %t110
  %t111 = add i64 0, 2
  %t112 = icmp eq i64 %t106, %t111
  %t105 = zext i1 %t112 to i8
  %t96 = and i8 %t97, %t105
  %pi113 = trunc i8 %t96 to i1
  br i1 %pi113, label %ps115, label %pp114
  pp114:
    unreachable
  ps115:
  call void @llvm.assume(i1 %pi113)
  %t118 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t119 = load i64, ptr %t118, align 8
  %t120 = add i64 0, 1
  %t116 = add nsw i64 %t119, %t120
  %t121 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t116, ptr %t121
  %t125 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t126 = load i64, ptr %t125, align 8
  %t127 = add i64 0, 5000000
  %t123 = srem i64 %t126, %t127
  %t128 = add i64 0, 4999999
  %t129 = icmp eq i64 %t123, %t128
  %t122 = zext i1 %t129 to i8
  %t131 = trunc i8 %t122 to i1
  br i1 %t131, label %guard.then130, label %guard.end130
  guard.then130:
    %t132 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
    %t133 = load i64, ptr %t132
    call void @txn_err_cold_0(i64 %t133)
    br label %guard.end130
  guard.end130:
  ret void
}
define void @txn_err_cold_0(i64 %__cp_count) local_unnamed_addr #0 {
  %t139 = add i64 0, 1
  %t137 = add nsw i64 %__cp_count, %t139
   %t136 = call i64 @__print_int(i64 %t137)
  %t141 = add i64 0, 10
   %t140 = call i64 @__print_char(i64 %t141)
  ret void
}


define void @txn_debug(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #11 alwaysinline {
  entry:
  %t145 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t146 = load i64, ptr %t145, align 8
  %t148 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t149 = load i64, ptr %t148, align 8
  %t150 = icmp slt i64 %t146, %t149
  %t143 = zext i1 %t150 to i8
  %t154 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t155 = load i64, ptr %t154, align 8
  %t156 = add i64 0, 8
  %t152 = srem i64 %t155, %t156
  %t157 = add i64 0, 3
  %t158 = icmp eq i64 %t152, %t157
  %t151 = zext i1 %t158 to i8
  %t142 = and i8 %t143, %t151
  %pi159 = trunc i8 %t142 to i1
  br i1 %pi159, label %ps161, label %pp160
  pp160:
    unreachable
  ps161:
  call void @llvm.assume(i1 %pi159)
  %t164 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t165 = load i64, ptr %t164, align 8
  %t166 = add i64 0, 1
  %t162 = add nsw i64 %t165, %t166
  %t167 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t162, ptr %t167
  %t171 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t172 = load i64, ptr %t171, align 8
  %t173 = add i64 0, 5000000
  %t169 = srem i64 %t172, %t173
  %t174 = add i64 0, 4999999
  %t175 = icmp eq i64 %t169, %t174
  %t168 = zext i1 %t175 to i8
  %t177 = trunc i8 %t168 to i1
  br i1 %t177, label %guard.then176, label %guard.end176
  guard.then176:
    %t178 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
    %t179 = load i64, ptr %t178
    call void @txn_debug_cold_0(i64 %t179)
    br label %guard.end176
  guard.end176:
  ret void
}
define void @txn_debug_cold_0(i64 %__cp_count) local_unnamed_addr #0 {
  %t185 = add i64 0, 1
  %t183 = add nsw i64 %__cp_count, %t185
   %t182 = call i64 @__print_int(i64 %t183)
  %t187 = add i64 0, 10
   %t186 = call i64 @__print_char(i64 %t187)
  ret void
}


define void @txn_data(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #11 alwaysinline {
  entry:
  %t191 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t192 = load i64, ptr %t191, align 8
  %t194 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t195 = load i64, ptr %t194, align 8
  %t196 = icmp slt i64 %t192, %t195
  %t189 = zext i1 %t196 to i8
  %t200 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t201 = load i64, ptr %t200, align 8
  %t202 = add i64 0, 8
  %t198 = srem i64 %t201, %t202
  %t203 = add i64 0, 4
  %t204 = icmp eq i64 %t198, %t203
  %t197 = zext i1 %t204 to i8
  %t188 = and i8 %t189, %t197
  %pi205 = trunc i8 %t188 to i1
  br i1 %pi205, label %ps207, label %pp206
  pp206:
    unreachable
  ps207:
  call void @llvm.assume(i1 %pi205)
  %t210 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t211 = load i64, ptr %t210, align 8
  %t212 = add i64 0, 1
  %t208 = add nsw i64 %t211, %t212
  %t213 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t208, ptr %t213
  %t217 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t218 = load i64, ptr %t217, align 8
  %t219 = add i64 0, 5000000
  %t215 = srem i64 %t218, %t219
  %t220 = add i64 0, 4999999
  %t221 = icmp eq i64 %t215, %t220
  %t214 = zext i1 %t221 to i8
  %t223 = trunc i8 %t214 to i1
  br i1 %t223, label %guard.then222, label %guard.end222
  guard.then222:
    %t224 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
    %t225 = load i64, ptr %t224
    call void @txn_data_cold_0(i64 %t225)
    br label %guard.end222
  guard.end222:
  ret void
}
define void @txn_data_cold_0(i64 %__cp_count) local_unnamed_addr #0 {
  %t231 = add i64 0, 1
  %t229 = add nsw i64 %__cp_count, %t231
   %t228 = call i64 @__print_int(i64 %t229)
  %t233 = add i64 0, 10
   %t232 = call i64 @__print_char(i64 %t233)
  ret void
}


define void @txn_ctrl(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #11 alwaysinline {
  entry:
  %t237 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t238 = load i64, ptr %t237, align 8
  %t240 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t241 = load i64, ptr %t240, align 8
  %t242 = icmp slt i64 %t238, %t241
  %t235 = zext i1 %t242 to i8
  %t246 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t247 = load i64, ptr %t246, align 8
  %t248 = add i64 0, 8
  %t244 = srem i64 %t247, %t248
  %t249 = add i64 0, 5
  %t250 = icmp eq i64 %t244, %t249
  %t243 = zext i1 %t250 to i8
  %t234 = and i8 %t235, %t243
  %pi251 = trunc i8 %t234 to i1
  br i1 %pi251, label %ps253, label %pp252
  pp252:
    unreachable
  ps253:
  call void @llvm.assume(i1 %pi251)
  %t256 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t257 = load i64, ptr %t256, align 8
  %t258 = add i64 0, 1
  %t254 = add nsw i64 %t257, %t258
  %t259 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t254, ptr %t259
  %t263 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t264 = load i64, ptr %t263, align 8
  %t265 = add i64 0, 5000000
  %t261 = srem i64 %t264, %t265
  %t266 = add i64 0, 4999999
  %t267 = icmp eq i64 %t261, %t266
  %t260 = zext i1 %t267 to i8
  %t269 = trunc i8 %t260 to i1
  br i1 %t269, label %guard.then268, label %guard.end268
  guard.then268:
    %t270 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
    %t271 = load i64, ptr %t270
    call void @txn_ctrl_cold_0(i64 %t271)
    br label %guard.end268
  guard.end268:
  ret void
}
define void @txn_ctrl_cold_0(i64 %__cp_count) local_unnamed_addr #0 {
  %t277 = add i64 0, 1
  %t275 = add nsw i64 %__cp_count, %t277
   %t274 = call i64 @__print_int(i64 %t275)
  %t279 = add i64 0, 10
   %t278 = call i64 @__print_char(i64 %t279)
  ret void
}


define void @txn_sync_(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #11 alwaysinline {
  entry:
  %t283 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t284 = load i64, ptr %t283, align 8
  %t286 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t287 = load i64, ptr %t286, align 8
  %t288 = icmp slt i64 %t284, %t287
  %t281 = zext i1 %t288 to i8
  %t292 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t293 = load i64, ptr %t292, align 8
  %t294 = add i64 0, 8
  %t290 = srem i64 %t293, %t294
  %t295 = add i64 0, 6
  %t296 = icmp eq i64 %t290, %t295
  %t289 = zext i1 %t296 to i8
  %t280 = and i8 %t281, %t289
  %pi297 = trunc i8 %t280 to i1
  br i1 %pi297, label %ps299, label %pp298
  pp298:
    unreachable
  ps299:
  call void @llvm.assume(i1 %pi297)
  %t302 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t303 = load i64, ptr %t302, align 8
  %t304 = add i64 0, 1
  %t300 = add nsw i64 %t303, %t304
  %t305 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t300, ptr %t305
  %t309 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t310 = load i64, ptr %t309, align 8
  %t311 = add i64 0, 5000000
  %t307 = srem i64 %t310, %t311
  %t312 = add i64 0, 4999999
  %t313 = icmp eq i64 %t307, %t312
  %t306 = zext i1 %t313 to i8
  %t315 = trunc i8 %t306 to i1
  br i1 %t315, label %guard.then314, label %guard.end314
  guard.then314:
    %t316 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
    %t317 = load i64, ptr %t316
    call void @txn_sync__cold_0(i64 %t317)
    br label %guard.end314
  guard.end314:
  ret void
}
define void @txn_sync__cold_0(i64 %__cp_count) local_unnamed_addr #0 {
  %t323 = add i64 0, 1
  %t321 = add nsw i64 %__cp_count, %t323
   %t320 = call i64 @__print_int(i64 %t321)
  %t325 = add i64 0, 10
   %t324 = call i64 @__print_char(i64 %t325)
  ret void
}


define void @txn_stat(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #11 alwaysinline {
  entry:
  %t329 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t330 = load i64, ptr %t329, align 8
  %t332 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t333 = load i64, ptr %t332, align 8
  %t334 = icmp slt i64 %t330, %t333
  %t327 = zext i1 %t334 to i8
  %t338 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t339 = load i64, ptr %t338, align 8
  %t340 = add i64 0, 8
  %t336 = srem i64 %t339, %t340
  %t341 = add i64 0, 7
  %t342 = icmp eq i64 %t336, %t341
  %t335 = zext i1 %t342 to i8
  %t326 = and i8 %t327, %t335
  %pi343 = trunc i8 %t326 to i1
  br i1 %pi343, label %ps345, label %pp344
  pp344:
    unreachable
  ps345:
  call void @llvm.assume(i1 %pi343)
  %t348 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t349 = load i64, ptr %t348, align 8
  %t350 = add i64 0, 1
  %t346 = add nsw i64 %t349, %t350
  %t351 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t346, ptr %t351
  %t355 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t356 = load i64, ptr %t355, align 8
  %t357 = add i64 0, 5000000
  %t353 = srem i64 %t356, %t357
  %t358 = add i64 0, 4999999
  %t359 = icmp eq i64 %t353, %t358
  %t352 = zext i1 %t359 to i8
  %t361 = trunc i8 %t352 to i1
  br i1 %t361, label %guard.then360, label %guard.end360
  guard.then360:
    %t362 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
    %t363 = load i64, ptr %t362
    call void @txn_stat_cold_0(i64 %t363)
    br label %guard.end360
  guard.end360:
  ret void
}
define void @txn_stat_cold_0(i64 %__cp_count) local_unnamed_addr #0 {
  %t369 = add i64 0, 1
  %t367 = add nsw i64 %__cp_count, %t369
   %t366 = call i64 @__print_int(i64 %t367)
  %t371 = add i64 0, 10
   %t370 = call i64 @__print_char(i64 %t371)
  ret void
}


define internal i8 @pre_ping(ptr noundef noalias nocapture align 8 %state) #10 {
  entry:
  %t3 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t4 = load i64, ptr %t3, align 8
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t7 = load i64, ptr %t6, align 8
  %t8 = icmp slt i64 %t4, %t7
  %t1 = zext i1 %t8 to i8
  %t12 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t13 = load i64, ptr %t12, align 8
  %t14 = add i64 0, 8
  %t10 = srem i64 %t13, %t14
  %t15 = add i64 0, 0
  %t16 = icmp eq i64 %t10, %t15
  %t9 = zext i1 %t16 to i8
  %t0 = and i8 %t1, %t9
  ret i8 %t0
}
define internal i8 @pre_ack(ptr noundef noalias nocapture align 8 %state) #10 {
  entry:
  %t3 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t4 = load i64, ptr %t3, align 8
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t7 = load i64, ptr %t6, align 8
  %t8 = icmp slt i64 %t4, %t7
  %t1 = zext i1 %t8 to i8
  %t12 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t13 = load i64, ptr %t12, align 8
  %t14 = add i64 0, 8
  %t10 = srem i64 %t13, %t14
  %t15 = add i64 0, 1
  %t16 = icmp eq i64 %t10, %t15
  %t9 = zext i1 %t16 to i8
  %t0 = and i8 %t1, %t9
  ret i8 %t0
}
define internal i8 @pre_err(ptr noundef noalias nocapture align 8 %state) #10 {
  entry:
  %t3 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t4 = load i64, ptr %t3, align 8
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t7 = load i64, ptr %t6, align 8
  %t8 = icmp slt i64 %t4, %t7
  %t1 = zext i1 %t8 to i8
  %t12 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t13 = load i64, ptr %t12, align 8
  %t14 = add i64 0, 8
  %t10 = srem i64 %t13, %t14
  %t15 = add i64 0, 2
  %t16 = icmp eq i64 %t10, %t15
  %t9 = zext i1 %t16 to i8
  %t0 = and i8 %t1, %t9
  ret i8 %t0
}
define internal i8 @pre_debug(ptr noundef noalias nocapture align 8 %state) #10 {
  entry:
  %t3 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t4 = load i64, ptr %t3, align 8
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t7 = load i64, ptr %t6, align 8
  %t8 = icmp slt i64 %t4, %t7
  %t1 = zext i1 %t8 to i8
  %t12 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t13 = load i64, ptr %t12, align 8
  %t14 = add i64 0, 8
  %t10 = srem i64 %t13, %t14
  %t15 = add i64 0, 3
  %t16 = icmp eq i64 %t10, %t15
  %t9 = zext i1 %t16 to i8
  %t0 = and i8 %t1, %t9
  ret i8 %t0
}
define internal i8 @pre_data(ptr noundef noalias nocapture align 8 %state) #10 {
  entry:
  %t3 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t4 = load i64, ptr %t3, align 8
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t7 = load i64, ptr %t6, align 8
  %t8 = icmp slt i64 %t4, %t7
  %t1 = zext i1 %t8 to i8
  %t12 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t13 = load i64, ptr %t12, align 8
  %t14 = add i64 0, 8
  %t10 = srem i64 %t13, %t14
  %t15 = add i64 0, 4
  %t16 = icmp eq i64 %t10, %t15
  %t9 = zext i1 %t16 to i8
  %t0 = and i8 %t1, %t9
  ret i8 %t0
}
define internal i8 @pre_ctrl(ptr noundef noalias nocapture align 8 %state) #10 {
  entry:
  %t3 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t4 = load i64, ptr %t3, align 8
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t7 = load i64, ptr %t6, align 8
  %t8 = icmp slt i64 %t4, %t7
  %t1 = zext i1 %t8 to i8
  %t12 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t13 = load i64, ptr %t12, align 8
  %t14 = add i64 0, 8
  %t10 = srem i64 %t13, %t14
  %t15 = add i64 0, 5
  %t16 = icmp eq i64 %t10, %t15
  %t9 = zext i1 %t16 to i8
  %t0 = and i8 %t1, %t9
  ret i8 %t0
}
define internal i8 @pre_sync_(ptr noundef noalias nocapture align 8 %state) #10 {
  entry:
  %t3 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t4 = load i64, ptr %t3, align 8
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t7 = load i64, ptr %t6, align 8
  %t8 = icmp slt i64 %t4, %t7
  %t1 = zext i1 %t8 to i8
  %t12 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t13 = load i64, ptr %t12, align 8
  %t14 = add i64 0, 8
  %t10 = srem i64 %t13, %t14
  %t15 = add i64 0, 6
  %t16 = icmp eq i64 %t10, %t15
  %t9 = zext i1 %t16 to i8
  %t0 = and i8 %t1, %t9
  ret i8 %t0
}
define internal i8 @pre_stat(ptr noundef noalias nocapture align 8 %state) #10 {
  entry:
  %t3 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t4 = load i64, ptr %t3, align 8
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t7 = load i64, ptr %t6, align 8
  %t8 = icmp slt i64 %t4, %t7
  %t1 = zext i1 %t8 to i8
  %t12 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t13 = load i64, ptr %t12, align 8
  %t14 = add i64 0, 8
  %t10 = srem i64 %t13, %t14
  %t15 = add i64 0, 7
  %t16 = icmp eq i64 %t10, %t15
  %t9 = zext i1 %t16 to i8
  %t0 = and i8 %t1, %t9
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
  store i64 0, ptr %ip_2, align 8
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
  store i64 0, ptr %t12, align 8
  br label %.mr_loop
.mr_loop:
  %mrp13 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %mrv14 = load i64, ptr %mrp13, align 8
  %t15 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t16 = load i64, ptr %t15, align 8
  %mrd17 = icmp sge i64 %mrv14, %t16
  br i1 %mrd17, label %.mr_end, label %.mr_cont
.mr_cont:
  %mrm18 = srem i64 %mrv14, 8
  %mre19 = icmp eq i64 %mrm18, 0
  br i1 %mre19, label %ping, label %.mr_next_0
ping:
  call void @txn_ping(ptr %state)
  br label %.mr_latch
.mr_next_0:
  %mre20 = icmp eq i64 %mrm18, 1
  br i1 %mre20, label %ack, label %.mr_next_1
ack:
  call void @txn_ack(ptr %state)
  br label %.mr_latch
.mr_next_1:
  %mre21 = icmp eq i64 %mrm18, 2
  br i1 %mre21, label %err, label %.mr_next_2
err:
  call void @txn_err(ptr %state)
  br label %.mr_latch
.mr_next_2:
  %mre22 = icmp eq i64 %mrm18, 3
  br i1 %mre22, label %debug, label %.mr_next_3
debug:
  call void @txn_debug(ptr %state)
  br label %.mr_latch
.mr_next_3:
  %mre23 = icmp eq i64 %mrm18, 4
  br i1 %mre23, label %data, label %.mr_next_4
data:
  call void @txn_data(ptr %state)
  br label %.mr_latch
.mr_next_4:
  %mre24 = icmp eq i64 %mrm18, 5
  br i1 %mre24, label %ctrl, label %.mr_next_5
ctrl:
  call void @txn_ctrl(ptr %state)
  br label %.mr_latch
.mr_next_5:
  %mre25 = icmp eq i64 %mrm18, 6
  br i1 %mre25, label %sync_, label %.mr_next_6
sync_:
  call void @txn_sync_(ptr %state)
  br label %.mr_latch
.mr_next_6:
  %mre26 = icmp eq i64 %mrm18, 7
  br i1 %mre26, label %stat, label %.mr_next_7
stat:
  call void @txn_stat(ptr %state)
  br label %.mr_latch
.mr_next_7:
  br label %.mr_latch
.mr_latch:
  %mrn27 = add nuw nsw i64 %mrv14, 1
  store i64 %mrn27, ptr %mrp13, align 8
  br label %.mr_loop
.mr_end:
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
