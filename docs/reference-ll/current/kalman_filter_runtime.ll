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
declare i64 @__print_float(float) #6
declare i64 @__print_int(i64) #6
declare { i64, i64 } @__getenv_briev({ i64, i64 }) #6
declare i64 @__print_char(i64) #6
declare i64 @__getenv_int({ i64, i64 }) #6
declare void @__print_str({ i64, i64 }) #6
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
@a00 = constant float bitcast (i32 1065353216 to float)
@a01 = constant float bitcast (i32 1008981770 to float)
@a02 = constant float bitcast (i32 944879383 to float)
@a10 = constant float bitcast (i32 0 to float)
@a11 = alias float, float* @a00
@a12 = alias float, float* @a01
@a20 = alias float, float* @a10
@a21 = alias float, float* @a10
@a22 = alias float, float* @a00
@q00 = constant float bitcast (i32 981668463 to float)
@q01 = alias float, float* @a10
@q02 = alias float, float* @a10
@q10 = alias float, float* @a10
@q11 = alias float, float* @q00
@q12 = alias float, float* @a10
@q20 = alias float, float* @a10
@q21 = alias float, float* @a10
@q22 = alias float, float* @q00

%StateChunk0 = type { i64, i64, float, float, float, float, float, float, float, float, float, float, float, float, i64 }
%State = type { i64, i64, float, float, float, float, float, float, float, float, float, float, float, float, i64 }
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

define void @txn_propagate(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #11 alwaysinline {
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
  %t25 = load float, ptr @a00
  %t27 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t28 = load float, ptr %t27, align 4
  %t24 = fmul fast float %t25, %t28
  %t30 = load float, ptr @a01
  %t32 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t33 = load float, ptr %t32, align 4
  %t29 = fmul fast float %t30, %t33
  %t23 = fadd fast float %t24, %t29
  %t35 = load float, ptr @a02
  %t37 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t38 = load float, ptr %t37, align 4
  %t34 = fmul fast float %t35, %t38
  %t22 = fadd fast float %t23, %t34
  %t42 = load float, ptr @a10
  %t44 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t45 = load float, ptr %t44, align 4
  %t41 = fmul fast float %t42, %t45
  %t47 = load float, ptr @a11
  %t49 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t50 = load float, ptr %t49, align 4
  %t46 = fmul fast float %t47, %t50
  %t40 = fadd fast float %t41, %t46
  %t52 = load float, ptr @a12
  %t54 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t55 = load float, ptr %t54, align 4
  %t51 = fmul fast float %t52, %t55
  %t39 = fadd fast float %t40, %t51
  %t59 = load float, ptr @a20
  %t61 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t62 = load float, ptr %t61, align 4
  %t58 = fmul fast float %t59, %t62
  %t64 = load float, ptr @a21
  %t66 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t67 = load float, ptr %t66, align 4
  %t63 = fmul fast float %t64, %t67
  %t57 = fadd fast float %t58, %t63
  %t69 = load float, ptr @a22
  %t71 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t72 = load float, ptr %t71, align 4
  %t68 = fmul fast float %t69, %t72
  %t56 = fadd fast float %t57, %t68
  %t76 = load float, ptr @a00
  %t78 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t79 = load float, ptr %t78, align 4
  %t75 = fmul fast float %t76, %t79
  %t81 = load float, ptr @a01
  %t83 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t84 = load float, ptr %t83, align 4
  %t80 = fmul fast float %t81, %t84
  %t74 = fadd fast float %t75, %t80
  %t86 = load float, ptr @a02
  %t88 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t89 = load float, ptr %t88, align 4
  %t85 = fmul fast float %t86, %t89
  %t73 = fadd fast float %t74, %t85
  %t93 = load float, ptr @a00
  %t95 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t96 = load float, ptr %t95, align 4
  %t92 = fmul fast float %t93, %t96
  %t98 = load float, ptr @a01
  %t100 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t101 = load float, ptr %t100, align 4
  %t97 = fmul fast float %t98, %t101
  %t91 = fadd fast float %t92, %t97
  %t103 = load float, ptr @a02
  %t105 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t106 = load float, ptr %t105, align 4
  %t102 = fmul fast float %t103, %t106
  %t90 = fadd fast float %t91, %t102
  %t110 = load float, ptr @a00
  %t112 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t113 = load float, ptr %t112, align 4
  %t109 = fmul fast float %t110, %t113
  %t115 = load float, ptr @a01
  %t117 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t118 = load float, ptr %t117, align 4
  %t114 = fmul fast float %t115, %t118
  %t108 = fadd fast float %t109, %t114
  %t120 = load float, ptr @a02
  %t122 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t123 = load float, ptr %t122, align 4
  %t119 = fmul fast float %t120, %t123
  %t107 = fadd fast float %t108, %t119
  %t127 = load float, ptr @a10
  %t129 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t130 = load float, ptr %t129, align 4
  %t126 = fmul fast float %t127, %t130
  %t132 = load float, ptr @a11
  %t134 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t135 = load float, ptr %t134, align 4
  %t131 = fmul fast float %t132, %t135
  %t125 = fadd fast float %t126, %t131
  %t137 = load float, ptr @a12
  %t139 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t140 = load float, ptr %t139, align 4
  %t136 = fmul fast float %t137, %t140
  %t124 = fadd fast float %t125, %t136
  %t144 = load float, ptr @a10
  %t146 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t147 = load float, ptr %t146, align 4
  %t143 = fmul fast float %t144, %t147
  %t149 = load float, ptr @a11
  %t151 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t152 = load float, ptr %t151, align 4
  %t148 = fmul fast float %t149, %t152
  %t142 = fadd fast float %t143, %t148
  %t154 = load float, ptr @a12
  %t156 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t157 = load float, ptr %t156, align 4
  %t153 = fmul fast float %t154, %t157
  %t141 = fadd fast float %t142, %t153
  %t161 = load float, ptr @a10
  %t163 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t164 = load float, ptr %t163, align 4
  %t160 = fmul fast float %t161, %t164
  %t166 = load float, ptr @a11
  %t168 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t169 = load float, ptr %t168, align 4
  %t165 = fmul fast float %t166, %t169
  %t159 = fadd fast float %t160, %t165
  %t171 = load float, ptr @a12
  %t173 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t174 = load float, ptr %t173, align 4
  %t170 = fmul fast float %t171, %t174
  %t158 = fadd fast float %t159, %t170
  %t178 = load float, ptr @a20
  %t180 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t181 = load float, ptr %t180, align 4
  %t177 = fmul fast float %t178, %t181
  %t183 = load float, ptr @a21
  %t185 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t186 = load float, ptr %t185, align 4
  %t182 = fmul fast float %t183, %t186
  %t176 = fadd fast float %t177, %t182
  %t188 = load float, ptr @a22
  %t190 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t191 = load float, ptr %t190, align 4
  %t187 = fmul fast float %t188, %t191
  %t175 = fadd fast float %t176, %t187
  %t195 = load float, ptr @a20
  %t197 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t198 = load float, ptr %t197, align 4
  %t194 = fmul fast float %t195, %t198
  %t200 = load float, ptr @a21
  %t202 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t203 = load float, ptr %t202, align 4
  %t199 = fmul fast float %t200, %t203
  %t193 = fadd fast float %t194, %t199
  %t205 = load float, ptr @a22
  %t207 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t208 = load float, ptr %t207, align 4
  %t204 = fmul fast float %t205, %t208
  %t192 = fadd fast float %t193, %t204
  %t212 = load float, ptr @a20
  %t214 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t215 = load float, ptr %t214, align 4
  %t211 = fmul fast float %t212, %t215
  %t217 = load float, ptr @a21
  %t219 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t220 = load float, ptr %t219, align 4
  %t216 = fmul fast float %t217, %t220
  %t210 = fadd fast float %t211, %t216
  %t222 = load float, ptr @a22
  %t224 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t225 = load float, ptr %t224, align 4
  %t221 = fmul fast float %t222, %t225
  %t209 = fadd fast float %t210, %t221
  %t228 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t229 = load i64, ptr %t228, align 8
  %t230 = add i64 0, 1
  %t226 = add nsw i64 %t229, %t230
  %t231 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 %t226, ptr %t231
  %t233 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store float %t22, ptr %t233
  %t235 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store float %t56, ptr %t235
  %t237 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store float %t39, ptr %t237
  %t243 = load float, ptr @a00
  %t241 = fmul fast float %t73, %t243
  %t246 = load float, ptr @a10
  %t244 = fmul fast float %t90, %t246
  %t240 = fadd fast float %t241, %t244
  %t249 = load float, ptr @a20
  %t247 = fmul fast float %t107, %t249
  %t239 = fadd fast float %t240, %t247
  %t250 = load float, ptr @q00
  %t238 = fadd fast float %t239, %t250
  %t251 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store float %t238, ptr %t251
  %t257 = load float, ptr @a00
  %t255 = fmul fast float %t124, %t257
  %t260 = load float, ptr @a10
  %t258 = fmul fast float %t141, %t260
  %t254 = fadd fast float %t255, %t258
  %t263 = load float, ptr @a20
  %t261 = fmul fast float %t158, %t263
  %t253 = fadd fast float %t254, %t261
  %t264 = load float, ptr @q10
  %t252 = fadd fast float %t253, %t264
  %t265 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store float %t252, ptr %t265
  %t271 = load float, ptr @a01
  %t269 = fmul fast float %t73, %t271
  %t274 = load float, ptr @a11
  %t272 = fmul fast float %t90, %t274
  %t268 = fadd fast float %t269, %t272
  %t277 = load float, ptr @a21
  %t275 = fmul fast float %t107, %t277
  %t267 = fadd fast float %t268, %t275
  %t278 = load float, ptr @q01
  %t266 = fadd fast float %t267, %t278
  %t279 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store float %t266, ptr %t279
  %t285 = load float, ptr @a01
  %t283 = fmul fast float %t124, %t285
  %t288 = load float, ptr @a11
  %t286 = fmul fast float %t141, %t288
  %t282 = fadd fast float %t283, %t286
  %t291 = load float, ptr @a21
  %t289 = fmul fast float %t158, %t291
  %t281 = fadd fast float %t282, %t289
  %t292 = load float, ptr @q11
  %t280 = fadd fast float %t281, %t292
  %t293 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store float %t280, ptr %t293
  %t299 = load float, ptr @a01
  %t297 = fmul fast float %t175, %t299
  %t302 = load float, ptr @a11
  %t300 = fmul fast float %t192, %t302
  %t296 = fadd fast float %t297, %t300
  %t305 = load float, ptr @a21
  %t303 = fmul fast float %t209, %t305
  %t295 = fadd fast float %t296, %t303
  %t306 = load float, ptr @q21
  %t294 = fadd fast float %t295, %t306
  %t307 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store float %t294, ptr %t307
  %t313 = load float, ptr @a02
  %t311 = fmul fast float %t175, %t313
  %t316 = load float, ptr @a12
  %t314 = fmul fast float %t192, %t316
  %t310 = fadd fast float %t311, %t314
  %t319 = load float, ptr @a22
  %t317 = fmul fast float %t209, %t319
  %t309 = fadd fast float %t310, %t317
  %t320 = load float, ptr @q22
  %t308 = fadd fast float %t309, %t320
  %t321 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  store float %t308, ptr %t321
  %t327 = load float, ptr @a02
  %t325 = fmul fast float %t124, %t327
  %t330 = load float, ptr @a12
  %t328 = fmul fast float %t141, %t330
  %t324 = fadd fast float %t325, %t328
  %t333 = load float, ptr @a22
  %t331 = fmul fast float %t158, %t333
  %t323 = fadd fast float %t324, %t331
  %t334 = load float, ptr @q12
  %t322 = fadd fast float %t323, %t334
  %t335 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store float %t322, ptr %t335
  %t341 = load float, ptr @a00
  %t339 = fmul fast float %t175, %t341
  %t344 = load float, ptr @a10
  %t342 = fmul fast float %t192, %t344
  %t338 = fadd fast float %t339, %t342
  %t347 = load float, ptr @a20
  %t345 = fmul fast float %t209, %t347
  %t337 = fadd fast float %t338, %t345
  %t348 = load float, ptr @q20
  %t336 = fadd fast float %t337, %t348
  %t349 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  store float %t336, ptr %t349
  %t355 = load float, ptr @a02
  %t353 = fmul fast float %t73, %t355
  %t358 = load float, ptr @a12
  %t356 = fmul fast float %t90, %t358
  %t352 = fadd fast float %t353, %t356
  %t361 = load float, ptr @a22
  %t359 = fmul fast float %t107, %t361
  %t351 = fadd fast float %t352, %t359
  %t362 = load float, ptr @q02
  %t350 = fadd fast float %t351, %t362
  %t363 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store float %t350, ptr %t363
  %t367 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t368 = load i64, ptr %t367, align 8
  %t369 = add i64 0, 5000000
  %t365 = srem i64 %t368, %t369
  %t370 = add i64 0, 0
  %t371 = icmp eq i64 %t365, %t370
  %t364 = zext i1 %t371 to i8
  %t373 = trunc i8 %t364 to i1
  br i1 %t373, label %guard.then372, label %guard.end372
  guard.then372:
    %t374 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
    %t375 = load float, ptr %t374
    %t376 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
    %t377 = load float, ptr %t376
    %t378 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
    %t379 = load float, ptr %t378
    %t380 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
    %t381 = load float, ptr %t380
    %t382 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
    %t383 = load float, ptr %t382
    %t384 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
    %t385 = load float, ptr %t384
    %t386 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
    %t387 = load float, ptr %t386
    %t388 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
    %t389 = load float, ptr %t388
    %t390 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
    %t391 = load float, ptr %t390
    %t392 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
    %t393 = load float, ptr %t392
    call void @txn_propagate_cold_0(float %t375, float %t377, float %t379, float %t381, float %t383, float %t385, float %t387, float %t389, float %t391, float %t393)
    br label %guard.end372
  guard.end372:
  ret void
}
define void @txn_propagate_cold_0(float %__cp_p00, float %__cp_p01, float %__cp_p02, float %__cp_p11, float %__cp_p12, float %__cp_p21, float %__cp_p22, float %__cp_x0, float %__cp_x1, float %__cp_x2) local_unnamed_addr #0 {
  %t398 = fmul fast float %__cp_x0, %__cp_x0
  %t397 = fadd fast float %t398, %__cp_p00
  %t396 = fadd fast float %t397, %__cp_p11
  %t395 = fadd fast float %t396, %__cp_p22
  %t406 = fmul fast float %__cp_x1, %__cp_x1
  %t405 = fadd fast float %t406, %__cp_p01
  %t404 = fadd fast float %t405, %__cp_p12
  %t413 = fmul fast float %__cp_x2, %__cp_x2
  %t412 = fadd fast float %t413, %__cp_p02
  %t411 = fadd fast float %t412, %__cp_p21
  %t421 = fadd fast float %t395, %t404
  %t420 = fadd fast float %t421, %t411
   %t419 = call i64 @__print_float(float %t420)
  %t426 = add i64 0, 10
   %t425 = call i64 @__print_char(i64 %t426)
  ret void
}


define internal i8 @pre_propagate(ptr noundef noalias nocapture align 8 %state) #10 {
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
  %ip_2b = bitcast i32 0 to float
  store float %ip_2b, ptr %ip_2, align 4
  %ip_3 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %ip_3b = bitcast i32 0 to float
  store float %ip_3b, ptr %ip_3, align 4
  %ip_4 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %ip_4b = bitcast i32 0 to float
  store float %ip_4b, ptr %ip_4, align 4
  %ip_5 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %ip_5b = bitcast i32 1036831949 to float
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
  %ip_9b = bitcast i32 1036831949 to float
  store float %ip_9b, ptr %ip_9, align 4
  %ip_10 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %ip_10b = bitcast i32 0 to float
  store float %ip_10b, ptr %ip_10, align 4
  %ip_11 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %ip_11b = bitcast i32 0 to float
  store float %ip_11b, ptr %ip_11, align 4
  %ip_12 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %ip_12b = bitcast i32 0 to float
  store float %ip_12b, ptr %ip_12, align 4
  %ip_13 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %ip_13b = bitcast i32 1036831949 to float
  store float %ip_13b, ptr %ip_13, align 4
  %ip_14 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  store i64 0, ptr %ip_14, align 8
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
  %ip_2b = bitcast i32 0 to float
  store float %ip_2b, ptr %t12, align 4
  %t13 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %ip_3b = bitcast i32 0 to float
  store float %ip_3b, ptr %t13, align 4
  %t14 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %ip_4b = bitcast i32 0 to float
  store float %ip_4b, ptr %t14, align 4
  %t15 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %ip_5b = bitcast i32 1036831949 to float
  store float %ip_5b, ptr %t15, align 4
  %t16 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %ip_6b = bitcast i32 0 to float
  store float %ip_6b, ptr %t16, align 4
  %t17 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %ip_7b = bitcast i32 0 to float
  store float %ip_7b, ptr %t17, align 4
  %t18 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %ip_8b = bitcast i32 0 to float
  store float %ip_8b, ptr %t18, align 4
  %t19 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %ip_9b = bitcast i32 1036831949 to float
  store float %ip_9b, ptr %t19, align 4
  %t20 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %ip_10b = bitcast i32 0 to float
  store float %ip_10b, ptr %t20, align 4
  %t21 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %ip_11b = bitcast i32 0 to float
  store float %ip_11b, ptr %t21, align 4
  %t22 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %ip_12b = bitcast i32 0 to float
  store float %ip_12b, ptr %t22, align 4
  %t23 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %ip_13b = bitcast i32 1036831949 to float
  store float %ip_13b, ptr %t23, align 4
  %t24 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  store i64 0, ptr %t24, align 8
  %clb26 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %whb25 = load i64, ptr %clb26, align 8
  br label %.wloop
.wloop:
  %t27 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t28 = load i64, ptr %t27, align 8
  %whd29 = icmp slt i64 %t28, %whb25
  br i1 %whd29, label %.wbody, label %.wend
.wbody:
  %t33 = load float, ptr @a00
  %t35 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t36 = load float, ptr %t35, align 4
  %t32 = fmul fast float %t33, %t36
  %t38 = load float, ptr @a01
  %t40 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t41 = load float, ptr %t40, align 4
  %t37 = fmul fast float %t38, %t41
  %t31 = fadd fast float %t32, %t37
  %t43 = load float, ptr @a02
  %t45 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t46 = load float, ptr %t45, align 4
  %t42 = fmul fast float %t43, %t46
  %t30 = fadd fast float %t31, %t42
  %t50 = load float, ptr @a10
  %t52 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t53 = load float, ptr %t52, align 4
  %t49 = fmul fast float %t50, %t53
  %t55 = load float, ptr @a11
  %t57 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t58 = load float, ptr %t57, align 4
  %t54 = fmul fast float %t55, %t58
  %t48 = fadd fast float %t49, %t54
  %t60 = load float, ptr @a12
  %t62 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t63 = load float, ptr %t62, align 4
  %t59 = fmul fast float %t60, %t63
  %t47 = fadd fast float %t48, %t59
  %t67 = load float, ptr @a20
  %t69 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t70 = load float, ptr %t69, align 4
  %t66 = fmul fast float %t67, %t70
  %t72 = load float, ptr @a21
  %t74 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t75 = load float, ptr %t74, align 4
  %t71 = fmul fast float %t72, %t75
  %t65 = fadd fast float %t66, %t71
  %t77 = load float, ptr @a22
  %t79 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t80 = load float, ptr %t79, align 4
  %t76 = fmul fast float %t77, %t80
  %t64 = fadd fast float %t65, %t76
  %t84 = load float, ptr @a00
  %t86 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t87 = load float, ptr %t86, align 4
  %t83 = fmul fast float %t84, %t87
  %t89 = load float, ptr @a01
  %t91 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t92 = load float, ptr %t91, align 4
  %t88 = fmul fast float %t89, %t92
  %t82 = fadd fast float %t83, %t88
  %t94 = load float, ptr @a02
  %t96 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t97 = load float, ptr %t96, align 4
  %t93 = fmul fast float %t94, %t97
  %t81 = fadd fast float %t82, %t93
  %t101 = load float, ptr @a00
  %t103 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t104 = load float, ptr %t103, align 4
  %t100 = fmul fast float %t101, %t104
  %t106 = load float, ptr @a01
  %t108 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t109 = load float, ptr %t108, align 4
  %t105 = fmul fast float %t106, %t109
  %t99 = fadd fast float %t100, %t105
  %t111 = load float, ptr @a02
  %t113 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t114 = load float, ptr %t113, align 4
  %t110 = fmul fast float %t111, %t114
  %t98 = fadd fast float %t99, %t110
  %t118 = load float, ptr @a00
  %t120 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t121 = load float, ptr %t120, align 4
  %t117 = fmul fast float %t118, %t121
  %t123 = load float, ptr @a01
  %t125 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t126 = load float, ptr %t125, align 4
  %t122 = fmul fast float %t123, %t126
  %t116 = fadd fast float %t117, %t122
  %t128 = load float, ptr @a02
  %t130 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t131 = load float, ptr %t130, align 4
  %t127 = fmul fast float %t128, %t131
  %t115 = fadd fast float %t116, %t127
  %t135 = load float, ptr @a10
  %t137 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t138 = load float, ptr %t137, align 4
  %t134 = fmul fast float %t135, %t138
  %t140 = load float, ptr @a11
  %t142 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t143 = load float, ptr %t142, align 4
  %t139 = fmul fast float %t140, %t143
  %t133 = fadd fast float %t134, %t139
  %t145 = load float, ptr @a12
  %t147 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t148 = load float, ptr %t147, align 4
  %t144 = fmul fast float %t145, %t148
  %t132 = fadd fast float %t133, %t144
  %t152 = load float, ptr @a10
  %t154 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t155 = load float, ptr %t154, align 4
  %t151 = fmul fast float %t152, %t155
  %t157 = load float, ptr @a11
  %t159 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t160 = load float, ptr %t159, align 4
  %t156 = fmul fast float %t157, %t160
  %t150 = fadd fast float %t151, %t156
  %t162 = load float, ptr @a12
  %t164 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t165 = load float, ptr %t164, align 4
  %t161 = fmul fast float %t162, %t165
  %t149 = fadd fast float %t150, %t161
  %t169 = load float, ptr @a10
  %t171 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t172 = load float, ptr %t171, align 4
  %t168 = fmul fast float %t169, %t172
  %t174 = load float, ptr @a11
  %t176 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t177 = load float, ptr %t176, align 4
  %t173 = fmul fast float %t174, %t177
  %t167 = fadd fast float %t168, %t173
  %t179 = load float, ptr @a12
  %t181 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t182 = load float, ptr %t181, align 4
  %t178 = fmul fast float %t179, %t182
  %t166 = fadd fast float %t167, %t178
  %t186 = load float, ptr @a20
  %t188 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t189 = load float, ptr %t188, align 4
  %t185 = fmul fast float %t186, %t189
  %t191 = load float, ptr @a21
  %t193 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t194 = load float, ptr %t193, align 4
  %t190 = fmul fast float %t191, %t194
  %t184 = fadd fast float %t185, %t190
  %t196 = load float, ptr @a22
  %t198 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t199 = load float, ptr %t198, align 4
  %t195 = fmul fast float %t196, %t199
  %t183 = fadd fast float %t184, %t195
  %t203 = load float, ptr @a20
  %t205 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t206 = load float, ptr %t205, align 4
  %t202 = fmul fast float %t203, %t206
  %t208 = load float, ptr @a21
  %t210 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t211 = load float, ptr %t210, align 4
  %t207 = fmul fast float %t208, %t211
  %t201 = fadd fast float %t202, %t207
  %t213 = load float, ptr @a22
  %t215 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t216 = load float, ptr %t215, align 4
  %t212 = fmul fast float %t213, %t216
  %t200 = fadd fast float %t201, %t212
  %t220 = load float, ptr @a20
  %t222 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t223 = load float, ptr %t222, align 4
  %t219 = fmul fast float %t220, %t223
  %t225 = load float, ptr @a21
  %t227 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t228 = load float, ptr %t227, align 4
  %t224 = fmul fast float %t225, %t228
  %t218 = fadd fast float %t219, %t224
  %t230 = load float, ptr @a22
  %t232 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t233 = load float, ptr %t232, align 4
  %t229 = fmul fast float %t230, %t233
  %t217 = fadd fast float %t218, %t229
  %t239 = load float, ptr @a00
  %t237 = fmul fast float %t81, %t239
  %t242 = load float, ptr @a10
  %t240 = fmul fast float %t98, %t242
  %t236 = fadd fast float %t237, %t240
  %t245 = load float, ptr @a20
  %t243 = fmul fast float %t115, %t245
  %t235 = fadd fast float %t236, %t243
  %t246 = load float, ptr @q00
  %t234 = fadd fast float %t235, %t246
  %cms247 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store float %t234, ptr %cms247, align 8
  %t253 = load float, ptr @a01
  %t251 = fmul fast float %t81, %t253
  %t256 = load float, ptr @a11
  %t254 = fmul fast float %t98, %t256
  %t250 = fadd fast float %t251, %t254
  %t259 = load float, ptr @a21
  %t257 = fmul fast float %t115, %t259
  %t249 = fadd fast float %t250, %t257
  %t260 = load float, ptr @q01
  %t248 = fadd fast float %t249, %t260
  %cms261 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store float %t248, ptr %cms261, align 8
  %t267 = load float, ptr @a02
  %t265 = fmul fast float %t81, %t267
  %t270 = load float, ptr @a12
  %t268 = fmul fast float %t98, %t270
  %t264 = fadd fast float %t265, %t268
  %t273 = load float, ptr @a22
  %t271 = fmul fast float %t115, %t273
  %t263 = fadd fast float %t264, %t271
  %t274 = load float, ptr @q02
  %t262 = fadd fast float %t263, %t274
  %cms275 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store float %t262, ptr %cms275, align 8
  %t281 = load float, ptr @a00
  %t279 = fmul fast float %t132, %t281
  %t284 = load float, ptr @a10
  %t282 = fmul fast float %t149, %t284
  %t278 = fadd fast float %t279, %t282
  %t287 = load float, ptr @a20
  %t285 = fmul fast float %t166, %t287
  %t277 = fadd fast float %t278, %t285
  %t288 = load float, ptr @q10
  %t276 = fadd fast float %t277, %t288
  %cms289 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store float %t276, ptr %cms289, align 8
  %t295 = load float, ptr @a01
  %t293 = fmul fast float %t132, %t295
  %t298 = load float, ptr @a11
  %t296 = fmul fast float %t149, %t298
  %t292 = fadd fast float %t293, %t296
  %t301 = load float, ptr @a21
  %t299 = fmul fast float %t166, %t301
  %t291 = fadd fast float %t292, %t299
  %t302 = load float, ptr @q11
  %t290 = fadd fast float %t291, %t302
  %cms303 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store float %t290, ptr %cms303, align 8
  %t309 = load float, ptr @a02
  %t307 = fmul fast float %t132, %t309
  %t312 = load float, ptr @a12
  %t310 = fmul fast float %t149, %t312
  %t306 = fadd fast float %t307, %t310
  %t315 = load float, ptr @a22
  %t313 = fmul fast float %t166, %t315
  %t305 = fadd fast float %t306, %t313
  %t316 = load float, ptr @q12
  %t304 = fadd fast float %t305, %t316
  %cms317 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store float %t304, ptr %cms317, align 8
  %t323 = load float, ptr @a00
  %t321 = fmul fast float %t183, %t323
  %t326 = load float, ptr @a10
  %t324 = fmul fast float %t200, %t326
  %t320 = fadd fast float %t321, %t324
  %t329 = load float, ptr @a20
  %t327 = fmul fast float %t217, %t329
  %t319 = fadd fast float %t320, %t327
  %t330 = load float, ptr @q20
  %t318 = fadd fast float %t319, %t330
  %cms331 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  store float %t318, ptr %cms331, align 8
  %t337 = load float, ptr @a01
  %t335 = fmul fast float %t183, %t337
  %t340 = load float, ptr @a11
  %t338 = fmul fast float %t200, %t340
  %t334 = fadd fast float %t335, %t338
  %t343 = load float, ptr @a21
  %t341 = fmul fast float %t217, %t343
  %t333 = fadd fast float %t334, %t341
  %t344 = load float, ptr @q21
  %t332 = fadd fast float %t333, %t344
  %cms345 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store float %t332, ptr %cms345, align 8
  %t351 = load float, ptr @a02
  %t349 = fmul fast float %t183, %t351
  %t354 = load float, ptr @a12
  %t352 = fmul fast float %t200, %t354
  %t348 = fadd fast float %t349, %t352
  %t357 = load float, ptr @a22
  %t355 = fmul fast float %t217, %t357
  %t347 = fadd fast float %t348, %t355
  %t358 = load float, ptr @q22
  %t346 = fadd fast float %t347, %t358
  %cms359 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  store float %t346, ptr %cms359, align 8
  %cms361 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store float %t30, ptr %cms361, align 8
  %cms363 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store float %t47, ptr %cms363, align 8
  %cms365 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store float %t64, ptr %cms365, align 8
  %t368 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t369 = load i64, ptr %t368, align 8
  %t370 = add i64 0, 1
  %t366 = add nsw i64 %t369, %t370
  %cms371 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 %t366, ptr %cms371, align 8
  %t375 = add i64 0, 5000000
  %t373 = srem i64 %t366, %t375
  %t376 = add i64 0, 0
  %t377 = icmp eq i64 %t373, %t376
  %t372 = zext i1 %t377 to i8
  %tb378 = trunc i8 %t372 to i1
  br i1 %tb378, label %.cmgb379, label %.cmgn379
.cmgb379:
  %t383 = fmul fast float %t30, %t30
  %t382 = fadd fast float %t383, %t234
  %t381 = fadd fast float %t382, %t290
  %t380 = fadd fast float %t381, %t346
  %t391 = fmul fast float %t47, %t47
  %t390 = fadd fast float %t391, %t248
  %t389 = fadd fast float %t390, %t304
  %t398 = fmul fast float %t64, %t64
  %t397 = fadd fast float %t398, %t262
  %t396 = fadd fast float %t397, %t332
  %t406 = fadd fast float %t380, %t389
  %t405 = fadd fast float %t406, %t396
   %t404 = call i64 @__print_float(float %t405)
  %t411 = add i64 0, 10
   %t410 = call i64 @__print_char(i64 %t411)
  br label %.cmgn379
.cmgn379:
  %whn412 = add nuw nsw i64 %t28, 1
  %t413 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 %whn412, ptr %t413, align 8
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
