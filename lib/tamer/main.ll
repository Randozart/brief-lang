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
declare i64 @atol(ptr) nounwind
declare ptr @getenv(ptr) nounwind
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
declare i64 @__getenv_int({ i64, i64 }) #6
declare i64 @__print_float(float) #6
declare { i64, i64 } @__getenv_briv({ i64, i64 }) #6
declare void @__print_str({ i64, i64 }) #6
declare i64 @__print_char(i64) #6
declare i64 @__print_int(i64) #6
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
%StateChunk0 = type { i64 }
%State = type { i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant <{ i64, [1 x i8] }> <{ i64 0, [1 x i8] c"\00" }>, align 8

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

define i64 @read_u8(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t4 = add i64 0, 8
  %t2 = sdiv i64 %arg1, %t4
  %t9 = add i64 0, 8
  %t7 = mul nsw i64 %t2, %t9
  %t5 = add nsw i64 %ac0, %t7
  %t13 = inttoptr i64 %t5 to ptr
  %t10 = load i64, ptr %t13, align 8
  %t16 = add i64 0, 8
  %t14 = srem i64 %arg1, %t16
  %t22 = add i64 0, 8
  %t20 = mul nsw i64 %t14, %t22
  %t18 = ashr i64 %t10, %t20
  %t23 = add i64 0, 255
  %t17 = and i64 %t18, %t23
  ret i64 %t17
}

define i64 @read_i8(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t3 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u8(ptr %state, ptr %t3, i64 %arg1)
  %t6 = add i64 0, 128
  %t7 = icmp sge i64 %t0, %t6
  %t4 = zext i1 %t7 to i8
  %t9 = trunc i8 %t4 to i1
  br i1 %t9, label %guard.then8, label %guard.end8
  guard.then8:
  %t12 = add i64 0, 256
  %t10 = sub nsw i64 %t0, %t12
  ret i64 %t10
  br label %guard.end8
  guard.end8:
  ret i64 %t0
}

define i64 @read_u16(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t4 = add i64 0, 8
  %t2 = sdiv i64 %arg1, %t4
  %t9 = add i64 0, 8
  %t7 = mul nsw i64 %t2, %t9
  %t5 = add nsw i64 %ac0, %t7
  %t13 = inttoptr i64 %t5 to ptr
  %t10 = load i64, ptr %t13, align 8
  %t16 = add i64 0, 8
  %t14 = srem i64 %arg1, %t16
  %t22 = add i64 0, 8
  %t20 = mul nsw i64 %t14, %t22
  %t18 = ashr i64 %t10, %t20
  %t23 = add i64 0, 65535
  %t17 = and i64 %t18, %t23
  ret i64 %t17
}

define i64 @read_i16(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t3 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u16(ptr %state, ptr %t3, i64 %arg1)
  %t6 = add i64 0, 32768
  %t7 = icmp sge i64 %t0, %t6
  %t4 = zext i1 %t7 to i8
  %t9 = trunc i8 %t4 to i1
  br i1 %t9, label %guard.then8, label %guard.end8
  guard.then8:
  %t12 = add i64 0, 65536
  %t10 = sub nsw i64 %t0, %t12
  ret i64 %t10
  br label %guard.end8
  guard.end8:
  ret i64 %t0
}

define i64 @read_u32(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t4 = add i64 0, 8
  %t2 = sdiv i64 %arg1, %t4
  %t9 = add i64 0, 8
  %t7 = mul nsw i64 %t2, %t9
  %t5 = add nsw i64 %ac0, %t7
  %t13 = inttoptr i64 %t5 to ptr
  %t10 = load i64, ptr %t13, align 8
  %t16 = add i64 0, 8
  %t14 = srem i64 %arg1, %t16
  %t22 = add i64 0, 8
  %t20 = mul nsw i64 %t14, %t22
  %t18 = ashr i64 %t10, %t20
  %t23 = add i64 0, 4294967295
  %t17 = and i64 %t18, %t23
  ret i64 %t17
}

define i64 @read_i64(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t4 = add i64 0, 8
  %t2 = sdiv i64 %arg1, %t4
  %t9 = add i64 0, 8
  %t7 = mul nsw i64 %t2, %t9
  %t5 = add nsw i64 %ac0, %t7
  %t13 = inttoptr i64 %t5 to ptr
  %t10 = load i64, ptr %t13, align 8
  %t16 = add i64 0, 8
  %t14 = srem i64 %arg1, %t16
  %t21 = add i64 0, 8
  %t19 = mul nsw i64 %t14, %t21
  %t17 = ashr i64 %t10, %t19
  ret i64 %t17
}

define i64 @lair_version(ptr noundef noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = add i64 0, 4
  %t3 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u32(ptr %state, ptr %t3, i64 %t2)
  ret i64 %t0
}

define i64 @lair_fn_offset(ptr noundef noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = add i64 0, 32
  %t3 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u64(ptr %t3, i64 %t2)
  ret i64 %t0
}

define i64 @lair_fn_size(ptr noundef noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = add i64 0, 40
  %t3 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u64(ptr %t3, i64 %t2)
  ret i64 %t0
}

define i64 @lair_bc_offset(ptr noundef noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = add i64 0, 48
  %t3 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u64(ptr %t3, i64 %t2)
  ret i64 %t0
}

define i64 @lair_bc_size(ptr noundef noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = add i64 0, 56
  %t3 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u64(ptr %t3, i64 %t2)
  ret i64 %t0
}

define i64 @fn_bc_offset(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t5 = add i64 0, 20
  %t3 = mul nsw i64 %arg1, %t5
  %t6 = add i64 0, 4
  %t2 = add nsw i64 %t3, %t6
  %t7 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_i64(ptr %state, ptr %t7, i64 %t2)
  ret i64 %t0
}

define i64 @fn_bc_len(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t5 = add i64 0, 20
  %t3 = mul nsw i64 %arg1, %t5
  %t6 = add i64 0, 12
  %t2 = add nsw i64 %t3, %t6
  %t7 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u32(ptr %state, ptr %t7, i64 %t2)
  ret i64 %t0
}

define i64 @fn_local_count(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t5 = add i64 0, 20
  %t3 = mul nsw i64 %arg1, %t5
  %t6 = add i64 0, 16
  %t2 = add nsw i64 %t3, %t6
  %t7 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u16(ptr %state, ptr %t7, i64 %t2)
  ret i64 %t0
}

define i64 @fn_arg_count(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t5 = add i64 0, 20
  %t3 = mul nsw i64 %arg1, %t5
  %t6 = add i64 0, 18
  %t2 = add nsw i64 %t3, %t6
  %t7 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u16(ptr %state, ptr %t7, i64 %t2)
  ret i64 %t0
}

define ptr @find_bounty_section(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = add i64 0, 17
  %t3 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u32(ptr %state, ptr %t3, i64 %t2)
  %t6 = add i64 0, 1
  %t7 = icmp sge i64 %t0, %t6
  %t4 = zext i1 %t7 to i8
  %t9 = trunc i8 %t4 to i1
  br i1 %t9, label %guard.then8, label %guard.end8
  guard.then8:
  %t13 = add i64 0, 21
  %t14 = inttoptr i64 %ac0 to ptr
  %t11 = call i64 @read_u8(ptr %state, ptr %t14, i64 %t13)
  %t16 = icmp eq i64 %t11, %arg1
  %t10 = zext i1 %t16 to i8
  %t18 = trunc i8 %t10 to i1
  br i1 %t18, label %guard.then17, label %guard.end17
  guard.then17:
  %t23 = add i64 0, 22
  %t24 = inttoptr i64 %ac0 to ptr
  %t21 = call i64 @read_u64(ptr %t24, i64 %t23)
  %t19 = add nsw i64 %ac0, %t21
  %t25 = inttoptr i64 %t19 to ptr
  ret ptr %t25
  br label %guard.end17
  guard.end17:
  br label %guard.end8
  guard.end8:
  %t31 = add i64 0, 2
  %t32 = icmp sge i64 %t0, %t31
  %t29 = zext i1 %t32 to i8
  %t34 = trunc i8 %t29 to i1
  br i1 %t34, label %guard.then33, label %guard.end33
  guard.then33:
  %t38 = add i64 0, 38
  %t39 = inttoptr i64 %ac0 to ptr
  %t36 = call i64 @read_u8(ptr %state, ptr %t39, i64 %t38)
  %t41 = icmp eq i64 %t36, %arg1
  %t35 = zext i1 %t41 to i8
  %t43 = trunc i8 %t35 to i1
  br i1 %t43, label %guard.then42, label %guard.end42
  guard.then42:
  %t48 = add i64 0, 39
  %t49 = inttoptr i64 %ac0 to ptr
  %t46 = call i64 @read_u64(ptr %t49, i64 %t48)
  %t44 = add nsw i64 %ac0, %t46
  %t50 = inttoptr i64 %t44 to ptr
  ret ptr %t50
  br label %guard.end42
  guard.end42:
  br label %guard.end33
  guard.end33:
  %t56 = add i64 0, 3
  %t57 = icmp sge i64 %t0, %t56
  %t54 = zext i1 %t57 to i8
  %t59 = trunc i8 %t54 to i1
  br i1 %t59, label %guard.then58, label %guard.end58
  guard.then58:
  %t63 = add i64 0, 55
  %t64 = inttoptr i64 %ac0 to ptr
  %t61 = call i64 @read_u8(ptr %state, ptr %t64, i64 %t63)
  %t66 = icmp eq i64 %t61, %arg1
  %t60 = zext i1 %t66 to i8
  %t68 = trunc i8 %t60 to i1
  br i1 %t68, label %guard.then67, label %guard.end67
  guard.then67:
  %t73 = add i64 0, 56
  %t74 = inttoptr i64 %ac0 to ptr
  %t71 = call i64 @read_u64(ptr %t74, i64 %t73)
  %t69 = add nsw i64 %ac0, %t71
  %t75 = inttoptr i64 %t69 to ptr
  ret ptr %t75
  br label %guard.end67
  guard.end67:
  br label %guard.end58
  guard.end58:
  %t79 = add i64 0, 0
  %t80 = inttoptr i64 %t79 to ptr
  ret ptr %t80
}

define i64 @opcode_stack_change(ptr noundef noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  ret i64 0
}

define i64 @instr_size(ptr noundef noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  ret i64 0
}

define i64 @analyze_fn_stack(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = add nsw i64 %arg1, %arg2
  %t6 = add i64 0, 0
  %t7 = add i64 0, 0
  %t8 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @analyze_fn_stack_loop(ptr %state, ptr %t8, i64 %t2, i64 %arg1, i64 %t6, i64 %t7)
  ret i64 %t0
}

define i64 @analyze_max_stack(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, ptr %arg2, i64 %arg3) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t7 = inttoptr i64 %ac0 to ptr
  %t8 = inttoptr i64 %ac2 to ptr
  %t0 = call i64 @analyze_max_stack_loop(ptr %state, ptr %t7, i64 %arg1, ptr %t8, i64 %arg3, i64 %t5, i64 %t6)
  %t11 = add i64 0, 1024
  %t12 = icmp sgt i64 %t0, %t11
  %t9 = zext i1 %t12 to i8
  %t14 = trunc i8 %t9 to i1
  br i1 %t14, label %guard.then13, label %guard.end13
  guard.then13:
  %t15 = add i64 0, 1024
  ret i64 %t15
  br label %guard.end13
  guard.end13:
  ret i64 %t0
}

define i64 @stack_slots(ptr noundef noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 64
  %t3 = icmp slt i64 %arg0, %t2
  %t0 = zext i1 %t3 to i8
  %t5 = trunc i8 %t0 to i1
  br i1 %t5, label %guard.then4, label %guard.end4
  guard.then4:
  %t6 = add i64 0, 64
  ret i64 %t6
  br label %guard.end4
  guard.end4:
  ret i64 %arg0
}

define i64 @locals_slots(ptr noundef noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 256
  %t3 = icmp slt i64 %arg0, %t2
  %t0 = zext i1 %t3 to i8
  %t5 = trunc i8 %t0 to i1
  br i1 %t5, label %guard.then4, label %guard.end4
  guard.then4:
  %t6 = add i64 0, 256
  ret i64 %t6
  br label %guard.end4
  guard.end4:
  %t11 = add i64 0, 2
  %t9 = mul nsw i64 %arg0, %t11
  ret i64 %t9
}

define i64 @frames_max(ptr noundef noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 16
  %t3 = icmp slt i64 %arg0, %t2
  %t0 = zext i1 %t3 to i8
  %t5 = trunc i8 %t0 to i1
  br i1 %t5, label %guard.then4, label %guard.end4
  guard.then4:
  %t6 = add i64 0, 16
  ret i64 %t6
  br label %guard.end4
  guard.end4:
  %t11 = add i64 0, 2
  %t9 = mul nsw i64 %arg0, %t11
  ret i64 %t9
}

define i64 @compute_stack_slots(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, ptr %arg2, i64 %arg3) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %t5 = inttoptr i64 %ac0 to ptr
  %t6 = inttoptr i64 %ac2 to ptr
  %t0 = call i64 @analyze_max_stack(ptr %state, ptr %t5, i64 %arg1, ptr %t6, i64 %arg3)
  %t9 = add i64 0, 64
  %t10 = icmp slt i64 %t0, %t9
  %t7 = zext i1 %t10 to i8
  %t12 = trunc i8 %t7 to i1
  br i1 %t12, label %guard.then11, label %guard.end11
  guard.then11:
  %t13 = add i64 0, 64
  ret i64 %t13
  br label %guard.end11
  guard.end11:
  ret i64 %t0
}

define i64 @compute_locals_slots(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, ptr %arg2, i64 %arg3) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %t5 = inttoptr i64 %ac0 to ptr
  %t6 = inttoptr i64 %ac2 to ptr
  %t0 = call i64 @analyze_max_stack(ptr %state, ptr %t5, i64 %arg1, ptr %t6, i64 %arg3)
  %t9 = add i64 0, 256
  %t10 = icmp slt i64 %t0, %t9
  %t7 = zext i1 %t10 to i8
  %t12 = trunc i8 %t7 to i1
  br i1 %t12, label %guard.then11, label %guard.end11
  guard.then11:
  %t13 = add i64 0, 256
  ret i64 %t13
  br label %guard.end11
  guard.end11:
  %t18 = add i64 0, 2
  %t16 = mul nsw i64 %t0, %t18
  ret i64 %t16
}

define i64 @compute_frames_max(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, ptr %arg2, i64 %arg3) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %t2 = add i64 0, 16
  %t3 = icmp slt i64 %arg1, %t2
  %t0 = zext i1 %t3 to i8
  %t5 = trunc i8 %t0 to i1
  br i1 %t5, label %guard.then4, label %guard.end4
  guard.then4:
  %t6 = add i64 0, 16
  ret i64 %t6
  br label %guard.end4
  guard.end4:
  %t12 = add i64 0, 2
  %t10 = mul nsw i64 %arg1, %t12
  %t13 = add i64 0, 256
  %t14 = icmp sgt i64 %t10, %t13
  %t9 = zext i1 %t14 to i8
  %t16 = trunc i8 %t9 to i1
  br i1 %t16, label %guard.then15, label %guard.end15
  guard.then15:
  %t17 = add i64 0, 256
  ret i64 %t17
  br label %guard.end15
  guard.end15:
  %t22 = add i64 0, 2
  %t20 = mul nsw i64 %arg1, %t22
  ret i64 %t20
}

define i64 @tame(ptr %arg0, i64 %arg1, ptr %arg2, i64 %arg3) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %t3 = add i64 0, 0
  %t1 = add nsw i64 %ac0, %t3
  %t4 = inttoptr i64 %t1 to ptr
  %t0 = load i64, ptr %t4, align 8
  %t7 = add i64 0, 4294967295
  %t5 = and i64 %t0, %t7
  %t10 = add i64 0, 1380532556
  %t11 = icmp ne i64 %t5, %t10
  %t8 = zext i1 %t11 to i8
  %t13 = trunc i8 %t8 to i1
  br i1 %t13, label %guard.then12, label %guard.end12
  guard.then12:
  %t14 = add i64 0, 101
  ret i64 %t14
  br label %guard.end12
  guard.end12:
  %t20 = add i64 0, 4
  %t18 = add nsw i64 %ac0, %t20
  %t21 = inttoptr i64 %t18 to ptr
  %t17 = load i64, ptr %t21, align 8
  %t26 = add i64 0, 5
  %t24 = add nsw i64 %ac0, %t26
  %t27 = inttoptr i64 %t24 to ptr
  %t23 = load i64, ptr %t27, align 8
  %t32 = add i64 0, 6
  %t30 = add nsw i64 %ac0, %t32
  %t33 = inttoptr i64 %t30 to ptr
  %t29 = load i64, ptr %t33, align 8
  %t38 = add i64 0, 7
  %t36 = add nsw i64 %ac0, %t38
  %t39 = inttoptr i64 %t36 to ptr
  %t35 = load i64, ptr %t39, align 8
  %t43 = add i64 0, 20
  %t41 = sdiv i64 %t23, %t43
  %t44 = add nsw i64 %ac0, %t29
  %t47 = add nsw i64 %ac0, %t17
  %t51 = add nsw i64 %t29, %t35
  %t55 = icmp sgt i64 %t51, %arg1
  %t50 = zext i1 %t55 to i8
  %t57 = trunc i8 %t50 to i1
  br i1 %t57, label %guard.then56, label %guard.end56
  guard.then56:
  %t58 = add i64 0, 103
  ret i64 %t58
  br label %guard.end56
  guard.end56:
  %t62 = add nsw i64 %t17, %t23
  %t66 = icmp sgt i64 %t62, %arg1
  %t61 = zext i1 %t66 to i8
  %t68 = trunc i8 %t61 to i1
  br i1 %t68, label %guard.then67, label %guard.end67
  guard.then67:
  %t69 = add i64 0, 104
  ret i64 %t69
  br label %guard.end67
  guard.end67:
  %t74 = add i64 0, 0
  %t75 = icmp eq i64 %t41, %t74
  %t72 = zext i1 %t75 to i8
  %t77 = trunc i8 %t72 to i1
  br i1 %t77, label %guard.then76, label %guard.end76
  guard.then76:
  %t78 = add i64 0, 105
  ret i64 %t78
  br label %guard.end76
  guard.end76:
  %t86 = inttoptr i64 %t47 to ptr
  %t87 = inttoptr i64 %t44 to ptr
  %t81 = call i64 @compute_stack_slots(ptr %state, ptr %t86, i64 %t41, ptr %t87, i64 %t35)
  %t93 = inttoptr i64 %t47 to ptr
  %t94 = inttoptr i64 %t44 to ptr
  %t88 = call i64 @compute_locals_slots(ptr %state, ptr %t93, i64 %t41, ptr %t94, i64 %t35)
  %t100 = inttoptr i64 %t47 to ptr
  %t101 = inttoptr i64 %t44 to ptr
  %t95 = call i64 @compute_frames_max(ptr %state, ptr %t100, i64 %t41, ptr %t101, i64 %t35)
  %t102 = add i64 0, 0
  ret i64 %t102
}

define i64 @analyze_fn_stack_loop(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2, i64 %arg3, i64 %arg4) local_unnamed_addr #8 {
  entry:
  %result = alloca i64, align 8
  store i64 0, ptr %result, align 8
  %ac0 = ptrtoint ptr %arg0 to i64
  %p0_s = alloca i64, align 8
  store i64 %ac0, ptr %p0_s, align 8
  %p1_s = alloca i64, align 8
  store i64 %arg1, ptr %p1_s, align 8
  %p2_s = alloca i64, align 8
  store i64 %arg2, ptr %p2_s, align 8
  %p3_s = alloca i64, align 8
  store i64 %arg3, ptr %p3_s, align 8
  %p4_s = alloca i64, align 8
  store i64 %arg4, ptr %p4_s, align 8
  br label %loop
loop:
  %p0_l104 = load i64, ptr %p0_s, align 8
  %p1_l105 = load i64, ptr %p1_s, align 8
  %p2_l106 = load i64, ptr %p2_s, align 8
  %p3_l107 = load i64, ptr %p3_s, align 8
  %p4_l108 = load i64, ptr %p4_s, align 8
  %t2 = load i64, ptr %p2_s, align 8
  %t4 = load i64, ptr %p1_s, align 8
  %t5 = icmp slt i64 %t2, %t4
  %t0 = zext i1 %t5 to i8
  %pc6 = trunc i8 %t0 to i1
  br i1 %pc6, label %body, label %done
body:
  %t9 = load i64, ptr %p0_s, align 8
  %t11 = load i64, ptr %p2_s, align 8
  %t12 = inttoptr i64 %t9 to ptr
  %t7 = call i64 @read_u8(ptr %state, ptr %t12, i64 %t11)
  %t13 = call i64 @opcode_stack_change(ptr %state, i64 %t7)
  %t17 = load i64, ptr %p3_s, align 8
  %t15 = add nsw i64 %t17, %t13
  store i64 %t15, ptr %p3_s
  %t21 = load i64, ptr %p3_s, align 8
  %t23 = load i64, ptr %p4_s, align 8
  %t24 = icmp sgt i64 %t21, %t23
  %t19 = zext i1 %t24 to i8
  %t26 = trunc i8 %t19 to i1
  br i1 %t26, label %guard.then25, label %guard.end25
  guard.then25:
  %t28 = load i64, ptr %p3_s, align 8
  store i64 %t28, ptr %p4_s
  br label %guard.end25
  guard.end25:
  %t32 = load i64, ptr %p3_s, align 8
  %t33 = add i64 0, 0
  %t34 = icmp slt i64 %t32, %t33
  %t30 = zext i1 %t34 to i8
  %t36 = trunc i8 %t30 to i1
  br i1 %t36, label %guard.then35, label %guard.end35
  guard.then35:
  %t37 = add i64 0, 0
  store i64 %t37, ptr %p3_s
  br label %guard.end35
  guard.end35:
  %t41 = load i64, ptr %p2_s, align 8
  %t42 = call i64 @instr_size(ptr %state, i64 %t7)
  %t39 = add nsw i64 %t41, %t42
  store i64 %t39, ptr %p2_s
  %t45 = load i64, ptr %p4_s, align 8
  store i64 %t45, ptr %result
  br label %post
post:
  br label %loop
done:
  %ret47 = load i64, ptr %result, align 8
  ret i64 %ret47
}

define i64 @analyze_max_stack_loop(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, ptr %arg2, i64 %arg3, i64 %arg4, i64 %arg5) local_unnamed_addr #8 {
  entry:
  %result = alloca i64, align 8
  store i64 0, ptr %result, align 8
  %ac0 = ptrtoint ptr %arg0 to i64
  %p0_s = alloca i64, align 8
  store i64 %ac0, ptr %p0_s, align 8
  %p1_s = alloca i64, align 8
  store i64 %arg1, ptr %p1_s, align 8
  %ac2 = ptrtoint ptr %arg2 to i64
  %p2_s = alloca i64, align 8
  store i64 %ac2, ptr %p2_s, align 8
  %p3_s = alloca i64, align 8
  store i64 %arg3, ptr %p3_s, align 8
  %p4_s = alloca i64, align 8
  store i64 %arg4, ptr %p4_s, align 8
  %p5_s = alloca i64, align 8
  store i64 %arg5, ptr %p5_s, align 8
  br label %loop
loop:
  %p0_l48 = load i64, ptr %p0_s, align 8
  %p1_l49 = load i64, ptr %p1_s, align 8
  %p2_l50 = load i64, ptr %p2_s, align 8
  %p3_l51 = load i64, ptr %p3_s, align 8
  %p4_l52 = load i64, ptr %p4_s, align 8
  %p5_l53 = load i64, ptr %p5_s, align 8
  %t2 = load i64, ptr %p4_s, align 8
  %t4 = load i64, ptr %p1_s, align 8
  %t5 = icmp slt i64 %t2, %t4
  %t0 = zext i1 %t5 to i8
  %pc6 = trunc i8 %t0 to i1
  br i1 %pc6, label %body, label %done
body:
  %t9 = load i64, ptr %p0_s, align 8
  %t11 = load i64, ptr %p4_s, align 8
  %t12 = inttoptr i64 %t9 to ptr
  %t7 = call i64 @fn_bc_offset(ptr %state, ptr %t12, i64 %t11)
  %t15 = load i64, ptr %p0_s, align 8
  %t17 = load i64, ptr %p4_s, align 8
  %t18 = inttoptr i64 %t15 to ptr
  %t13 = call i64 @fn_bc_len(ptr %state, ptr %t18, i64 %t17)
  %t21 = load i64, ptr %p0_s, align 8
  %t23 = load i64, ptr %p4_s, align 8
  %t24 = inttoptr i64 %t21 to ptr
  %t19 = call i64 @fn_local_count(ptr %state, ptr %t24, i64 %t23)
  %t27 = load i64, ptr %p2_s, align 8
  %t30 = inttoptr i64 %t27 to ptr
  %t25 = call i64 @analyze_fn_stack(ptr %state, ptr %t30, i64 %t7, i64 %t13)
  %t31 = add nsw i64 %t25, %t19
  %t37 = load i64, ptr %p5_s, align 8
  %t38 = icmp sgt i64 %t31, %t37
  %t34 = zext i1 %t38 to i8
  %t40 = trunc i8 %t34 to i1
  br i1 %t40, label %guard.then39, label %guard.end39
  guard.then39:
  store i64 %t31, ptr %p5_s
  br label %guard.end39
  guard.end39:
  %t45 = load i64, ptr %p4_s, align 8
  %t46 = add i64 0, 1
  %t43 = add nsw i64 %t45, %t46
  store i64 %t43, ptr %p4_s
  %t48 = load i64, ptr %p5_s, align 8
  store i64 %t48, ptr %result
  br label %post
post:
  br label %loop
done:
  %ret50 = load i64, ptr %result, align 8
  ret i64 %ret50
}

define void @init_state(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #0 {
  entry:
  %ip_0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 0, ptr %ip_0, align 8
  ret void
}


define i32 @main() local_unnamed_addr #0 {
entry:
  %state = alloca %State, align 8
  %t0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 0, ptr %t0, align 8
  %sa1 = alloca i64, align 8
  %any_active_2 = alloca i64, align 8
  br label %.ss_main_loop
.ss_main_loop:
  store i64 0, ptr %any_active_2
  %t3 = load i64, ptr %any_active_2
  %t4 = icmp eq i64 %t3, 0
  br i1 %t4, label %.end, label %.ss_main_loop
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

!0 = !{!"Briv"}
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
