; ModuleID = 'program.ll'
source_filename = "program.bv"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

%Frame = type { i64, i64, i64 }
%HCallArgs = type { i64, i64, i64, i64, i64 }
%HashMap = type { ptr, ptr, ptr, i64, i64 }
%HostTable = type { [64 x i64], [64 x i64], i64 }
%List = type { { ptr }, i64, i64 }
%ListBuffer = type { ptr }
%PiggyBank = type { i64 }
%RingBuffer = type { [256 x i64], i64, i64 }
%Slice = type { ptr, i64 }
%Stack = type { i64, i64 }
%VMFrames = type { [256 x ptr], i64 }
%VMLocals = type { [4096 x i64], i64 }
%VMStack = type { [1024 x i64], i64 }

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
declare i64 @briev_symbol_available(ptr)
declare void @__briev_free(ptr) nounwind argmemonly
declare i64 @__briev_now() nounwind
declare ptr @realloc(ptr, i64) nounwind
declare i64 @ShellCmd(i64)
declare i64 @briev_syscall(i64, i64, i64, i64, i64, i64, i64)
declare i64 @briev_sysconf(i64)
declare ptr @dlopen(ptr, i32) nounwind
declare ptr @dlsym(ptr, ptr) nounwind
declare i32 @dlclose(ptr) nounwind
declare i64 @briev_backtrace()
declare ptr @__argv_command() #6
declare i64 @__argv_count() #6
declare ptr @__argv_get(i64) #6
declare i8 @__argv_has(ptr) #6
declare ptr @__argv_value(ptr) #6
declare i64 @__eprint_str(ptr) #6
declare ptr @__getenv_briev(ptr) #6
declare i64 @__getenv_int(ptr) #6
declare i64 @__print_bool(i64) #6
declare i64 @__print_char(i64) #6
declare i64 @__print_float(float) #6
declare i64 @__print_float64(double) #6
declare i64 @__print_int(i64) #6
declare i8* @__chr_to_str(i32) #1
declare ptr @int_to_str(i64) #1
declare ptr @uint_to_str(i64) #1
declare ptr @float_to_str(float) #1
declare ptr @bool_to_str(i64) #1
declare ptr @char_to_str(i64) #1
declare i64 @str_to_int(ptr) #1
declare i64 @str_to_uint(ptr) #1
declare float @str_to_float(ptr) #1
declare i64 @str_to_bool(ptr) #1
declare i64 @str_first_char(ptr) #1
declare i64 @__str_bytes__(i64) #1
declare i64 @__briev_coll_resize(i64, i64) #1
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
declare void @__exit(i64) #6
declare i64 @briev_futex(i64, i64, i64, i64, i64, i64) #1
declare i64 @__ioctl__(i64, i64, i64) #1
declare i64 @__isatty__(i64) #1
declare i64 @__print(ptr) #1
declare i64 @__print_str(ptr) #1
declare ptr @briev_str_substr(ptr, i64, i64) #1
declare i64 @briev_str_eq(ptr, ptr) #1
declare ptr @briev_str_band(ptr, ptr) #1
declare ptr @briev_str_bor(ptr, ptr) #1
declare ptr @briev_str_bxor(ptr, ptr) #1
declare ptr @briev_str_bnot(ptr) #1
declare ptr @briev_bits_to_str(ptr) #1
declare i64 @briev_char_len(ptr) #1
declare i64 @briev_str_next_char(ptr, ptr) #1
declare ptr @briev_mask_select(ptr, ptr, i64) #1
declare ptr @briev_mask_select64(ptr, i64, ptr, i64) #1
declare ptr @briev_mask_select64_i8mask(ptr, i64, ptr, i64) #1
declare ptr @briev_mask_select_f32(ptr, i64, ptr, i64) #1
declare ptr @briev_mask_select_f32_i8mask(ptr, i64, ptr, i64) #1
declare ptr @briev_slice_range64(ptr, i64, i64, i64) #1
declare ptr @briev_slice_range_f32(ptr, i64, i64, i64) #1
@__briev_argc = global i32 0
@__briev_argv = global ptr null
@__briev_cancel_flag = global i32 0
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
@SYS_READ = constant i64 0
@SYS_WRITE = constant i64 1

%StateChunk0 = type { i64 }
%State = type { i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant <{ i64, [1 x i8] }> <{ i64 0, [1 x i8] c"\00" }>, align 8
@str.1 = private unnamed_addr constant <{ i64, [141 x i8] }> <{ i64 140, [141 x i8] c"a PiggyBank is opaque \e2\80\94 individual elements cannot be read out. Smash it to extract everything at once: `let all: List<K>; all ~<- piggy;`\00" }>, align 8


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

define ptr @entry_cmd(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #8 {
  entry:
   %t0 = call ptr @__argv_command()
  ret ptr %t0
}

define i8 @arg_present(ptr noundef noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
    %t2 = inttoptr i64 %ac0 to ptr
   %t0 = call i8 @__argv_has(ptr %t2)
  ret i8 %t0
}

define i64 @read_u8(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t4 = add i64 0, 8
  %t2 = sdiv i64 %arg1, %t4
  %t5 = inttoptr i64 %ac0 to ptr
  %t6 = add i64 %t2, 0
  %t7 = getelementptr i64, ptr %t5, i64 %t6
  %t0 = load i64, ptr %t7
  %t10 = add i64 0, 8
  %t8 = srem i64 %arg1, %t10
  %t16 = add i64 0, 8
  %t14 = mul nsw i64 %t8, %t16
  %t12 = ashr i64 %t0, %t14
  %t17 = add i64 0, 255
  %t11 = and i64 %t12, %t17
  ret i64 %t11
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
  guard.end8:
  ret i64 %t0
}

define i64 @read_u16(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t4 = add i64 0, 8
  %t2 = sdiv i64 %arg1, %t4
  %t5 = inttoptr i64 %ac0 to ptr
  %t6 = add i64 %t2, 0
  %t7 = getelementptr i64, ptr %t5, i64 %t6
  %t0 = load i64, ptr %t7
  %t10 = add i64 0, 8
  %t8 = srem i64 %arg1, %t10
  %t16 = add i64 0, 8
  %t14 = mul nsw i64 %t8, %t16
  %t12 = ashr i64 %t0, %t14
  %t17 = add i64 0, 65535
  %t11 = and i64 %t12, %t17
  ret i64 %t11
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
  guard.end8:
  ret i64 %t0
}

define i64 @read_u32(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t4 = add i64 0, 8
  %t2 = sdiv i64 %arg1, %t4
  %t5 = inttoptr i64 %ac0 to ptr
  %t6 = add i64 %t2, 0
  %t7 = getelementptr i64, ptr %t5, i64 %t6
  %t0 = load i64, ptr %t7
  %t10 = add i64 0, 8
  %t8 = srem i64 %arg1, %t10
  %t16 = add i64 0, 8
  %t14 = mul nsw i64 %t8, %t16
  %t12 = ashr i64 %t0, %t14
  %t17 = add i64 0, 4294967295
  %t11 = and i64 %t12, %t17
  ret i64 %t11
}

define i64 @read_i64(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t4 = add i64 0, 8
  %t2 = sdiv i64 %arg1, %t4
  %t5 = inttoptr i64 %ac0 to ptr
  %t6 = add i64 %t2, 0
  %t7 = getelementptr i64, ptr %t5, i64 %t6
  %t0 = load i64, ptr %t7
  %t10 = add i64 0, 8
  %t8 = srem i64 %arg1, %t10
  %t15 = add i64 0, 8
  %t13 = mul nsw i64 %t8, %t15
  %t11 = ashr i64 %t0, %t13
  ret i64 %t11
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

define i64 @fn_bc_len(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t7 = add i64 0, 20
  %t5 = mul nsw i64 %arg2, %t7
  %t3 = add nsw i64 %arg1, %t5
  %t8 = add i64 0, 12
  %t2 = add nsw i64 %t3, %t8
  %t9 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u32(ptr %state, ptr %t9, i64 %t2)
  ret i64 %t0
}

define i64 @fn_local_count(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t7 = add i64 0, 20
  %t5 = mul nsw i64 %arg2, %t7
  %t3 = add nsw i64 %arg1, %t5
  %t8 = add i64 0, 16
  %t2 = add nsw i64 %t3, %t8
  %t9 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u16(ptr %state, ptr %t9, i64 %t2)
  ret i64 %t0
}

define i64 @fn_arg_count(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t7 = add i64 0, 20
  %t5 = mul nsw i64 %arg2, %t7
  %t3 = add nsw i64 %arg1, %t5
  %t8 = add i64 0, 18
  %t2 = add nsw i64 %t3, %t8
  %t9 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u16(ptr %state, ptr %t9, i64 %t2)
  ret i64 %t0
}

define i64 @host_arity(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t3 = add i64 0, 0
  %t4 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @host_arity_scan(ptr %state, ptr %t4, i64 %arg1, i64 %t3)
  ret i64 %t0
}

define i64 @host_dispatch1(ptr noundef noalias nocapture align 8 %state, i64 %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %t2 = load i64, ptr @HOST_ID_PRINT_INT
  %t3 = icmp eq i64 %arg0, %t2
  %t0 = zext i1 %t3 to i8
  %t5 = trunc i8 %t0 to i1
  br i1 %t5, label %guard.then4, label %guard.end4
  guard.then4:
  %t6 = call i64 @BrievHostPrintInt(i64 %arg1)
  ret i64 %t6
  guard.end4:
  %t12 = load i64, ptr @HOST_ID_LOG_INT
  %t13 = icmp eq i64 %arg0, %t12
  %t10 = zext i1 %t13 to i8
  %t15 = trunc i8 %t10 to i1
  br i1 %t15, label %guard.then14, label %guard.end14
  guard.then14:
  %t16 = call i64 @BrievHostPrintInt(i64 %arg1)
  ret i64 %t16
  guard.end14:
  %t22 = load i64, ptr @HOST_ID_UNKNOWN_MIN
  %t23 = icmp sge i64 %arg0, %t22
  %t20 = zext i1 %t23 to i8
  %t25 = trunc i8 %t20 to i1
  br i1 %t25, label %guard.then24, label %guard.end24
  guard.then24:
  %t26 = call i64 @BrievHostFail(i64 %arg0, i64 %arg1)
  ret i64 %t26
  guard.end24:
  %t31 = call i64 @BrievHostFail(i64 %arg0, i64 %arg1)
  ret i64 %t31
}

define i64 @vm_loop(ptr noundef noalias nocapture align 8 %state, ptr %arg0, ptr %arg1, ptr %arg2, ptr %arg3, i64 %arg4, ptr %arg5, i64 %arg6, i64 %arg7, ptr %arg8, i64 %arg9, i64 %arg10, i64 %arg11) local_unnamed_addr #8 {
  entry:
  %result = alloca i64, align 8
  store i64 0, ptr %result, align 8
  %ac0 = ptrtoint ptr %arg0 to i64
  %p0_s = alloca i64, align 8
  store i64 %ac0, ptr %p0_s, align 8
  %ac1 = ptrtoint ptr %arg1 to i64
  %p1_s = alloca i64, align 8
  store i64 %ac1, ptr %p1_s, align 8
  %ac2 = ptrtoint ptr %arg2 to i64
  %p2_s = alloca i64, align 8
  store i64 %ac2, ptr %p2_s, align 8
  %ac3 = ptrtoint ptr %arg3 to i64
  %p3_s = alloca i64, align 8
  store i64 %ac3, ptr %p3_s, align 8
  %p4_s = alloca i64, align 8
  store i64 %arg4, ptr %p4_s, align 8
  %ac5 = ptrtoint ptr %arg5 to i64
  %p5_s = alloca i64, align 8
  store i64 %ac5, ptr %p5_s, align 8
  %p6_s = alloca i64, align 8
  store i64 %arg6, ptr %p6_s, align 8
  %p7_s = alloca i64, align 8
  store i64 %arg7, ptr %p7_s, align 8
  %ac8 = ptrtoint ptr %arg8 to i64
  %p8_s = alloca i64, align 8
  store i64 %ac8, ptr %p8_s, align 8
  %p9_s = alloca i64, align 8
  store i64 %arg9, ptr %p9_s, align 8
  %p10_s = alloca i64, align 8
  store i64 %arg10, ptr %p10_s, align 8
  %p11_s = alloca i64, align 8
  store i64 %arg11, ptr %p11_s, align 8
  br label %loop
loop:
  %p0_l35 = load i64, ptr %p0_s, align 8
  %p1_l36 = load i64, ptr %p1_s, align 8
  %p2_l37 = load i64, ptr %p2_s, align 8
  %p3_l38 = load i64, ptr %p3_s, align 8
  %p4_l39 = load i64, ptr %p4_s, align 8
  %p5_l40 = load i64, ptr %p5_s, align 8
  %p6_l41 = load i64, ptr %p6_s, align 8
  %p7_l42 = load i64, ptr %p7_s, align 8
  %p8_l43 = load i64, ptr %p8_s, align 8
  %p9_l44 = load i64, ptr %p9_s, align 8
  %p10_l45 = load i64, ptr %p10_s, align 8
  %p11_l46 = load i64, ptr %p11_s, align 8
  %t4 = load i64, ptr %p9_s, align 8
  %t5 = add i64 0, 0
  %t6 = icmp sge i64 %t4, %t5
  %t2 = zext i1 %t6 to i8
  %t9 = load i64, ptr %p9_s, align 8
  %t11 = load i64, ptr %p4_s, align 8
  %t12 = icmp slt i64 %t9, %t11
  %t7 = zext i1 %t12 to i8
  %t1 = and i8 %t2, %t7
  %t15 = load i64, ptr %p11_s, align 8
  %t16 = add i64 0, 0
  %t17 = icmp ne i64 %t15, %t16
  %t13 = zext i1 %t17 to i8
  %t0 = and i8 %t1, %t13
  %pc18 = trunc i8 %t0 to i1
  br i1 %pc18, label %body, label %done
body:
  %t21 = load i64, ptr %p3_s, align 8
  %t23 = load i64, ptr %p9_s, align 8
  %t24 = inttoptr i64 %t21 to ptr
  %t19 = call i64 @read_u8(ptr %state, ptr %t24, i64 %t23)
  %t27 = load i64, ptr %p0_s, align 8
  %t29 = load i64, ptr %p1_s, align 8
  %t31 = load i64, ptr %p2_s, align 8
  %t33 = load i64, ptr %p3_s, align 8
  %t35 = load i64, ptr %p5_s, align 8
  %t37 = load i64, ptr %p6_s, align 8
  %t39 = load i64, ptr %p7_s, align 8
  %t41 = load i64, ptr %p8_s, align 8
  %t44 = load i64, ptr %p9_s, align 8
  %t46 = load i64, ptr %p10_s, align 8
  %t47 = inttoptr i64 %t27 to ptr
  %t48 = inttoptr i64 %t29 to ptr
  %t49 = inttoptr i64 %t31 to ptr
  %t50 = inttoptr i64 %t33 to ptr
  %t51 = inttoptr i64 %t35 to ptr
  %t52 = inttoptr i64 %t41 to ptr
  %t25 = call i64 @exec_op(ptr %state, ptr %t47, ptr %t48, ptr %t49, ptr %t50, ptr %t51, i64 %t37, i64 %t39, ptr %t52, i64 %t19, i64 %t44, i64 %t46)
  %t55 = add i64 0, 0
  %t56 = icmp slt i64 %t25, %t55
  %t53 = zext i1 %t56 to i8
  %t58 = trunc i8 %t53 to i1
  br i1 %t58, label %guard.then57, label %guard.end57
  guard.then57:
  %t59 = add i64 0, 0
  store i64 %t59, ptr %p11_s
  br label %guard.end57
  guard.end57:
  %t64 = add i64 0, 0
  %t65 = icmp sge i64 %t25, %t64
  %t62 = zext i1 %t65 to i8
  %t69 = load i64, ptr %p4_s, align 8
  %t70 = icmp slt i64 %t25, %t69
  %t66 = zext i1 %t70 to i8
  %t61 = and i8 %t62, %t66
  %t72 = trunc i8 %t61 to i1
  br i1 %t72, label %guard.then71, label %guard.end71
  guard.then71:
  store i64 %t25, ptr %p9_s
  br label %guard.end71
  guard.end71:
  %t75 = add i64 0, 0
  store i64 %t75, ptr %result
  br label %post
post:
  br label %loop
done:
  %ret77 = load i64, ptr %result, align 8
  ret i64 %ret77
}

define i64 @exec_op(ptr noundef noalias nocapture align 8 %state, ptr %arg0, ptr %arg1, ptr %arg2, ptr %arg3, ptr %arg4, i64 %arg5, i64 %arg6, ptr %arg7, i64 %arg8, i64 %arg9, i64 %arg10) local_unnamed_addr #8 alwaysinline {
  entry:
  %result = alloca i64, align 8
  store i64 0, ptr %result, align 8
  %ac0 = ptrtoint ptr %arg0 to i64
  %p0_s = alloca i64, align 8
  store i64 %ac0, ptr %p0_s, align 8
  %ac1 = ptrtoint ptr %arg1 to i64
  %p1_s = alloca i64, align 8
  store i64 %ac1, ptr %p1_s, align 8
  %ac2 = ptrtoint ptr %arg2 to i64
  %p2_s = alloca i64, align 8
  store i64 %ac2, ptr %p2_s, align 8
  %ac3 = ptrtoint ptr %arg3 to i64
  %p3_s = alloca i64, align 8
  store i64 %ac3, ptr %p3_s, align 8
  %ac4 = ptrtoint ptr %arg4 to i64
  %p4_s = alloca i64, align 8
  store i64 %ac4, ptr %p4_s, align 8
  %p5_s = alloca i64, align 8
  store i64 %arg5, ptr %p5_s, align 8
  %p6_s = alloca i64, align 8
  store i64 %arg6, ptr %p6_s, align 8
  %ac7 = ptrtoint ptr %arg7 to i64
  %p7_s = alloca i64, align 8
  store i64 %ac7, ptr %p7_s, align 8
  %p8_s = alloca i64, align 8
  store i64 %arg8, ptr %p8_s, align 8
  %p9_s = alloca i64, align 8
  store i64 %arg9, ptr %p9_s, align 8
  %p10_s = alloca i64, align 8
  store i64 %arg10, ptr %p10_s, align 8
  br label %loop
loop:
  %p0_l78 = load i64, ptr %p0_s, align 8
  %p1_l79 = load i64, ptr %p1_s, align 8
  %p2_l80 = load i64, ptr %p2_s, align 8
  %p3_l81 = load i64, ptr %p3_s, align 8
  %p4_l82 = load i64, ptr %p4_s, align 8
  %p5_l83 = load i64, ptr %p5_s, align 8
  %p6_l84 = load i64, ptr %p6_s, align 8
  %p7_l85 = load i64, ptr %p7_s, align 8
  %p8_l86 = load i64, ptr %p8_s, align 8
  %p9_l87 = load i64, ptr %p9_s, align 8
  %p10_l88 = load i64, ptr %p10_s, align 8
  %t4 = load i64, ptr %p8_s, align 8
  %t5 = add i64 0, 0
  %t6 = icmp sge i64 %t4, %t5
  %t2 = zext i1 %t6 to i8
  %t9 = load i64, ptr %p8_s, align 8
  %t10 = add i64 0, 255
  %t11 = icmp sle i64 %t9, %t10
  %t7 = zext i1 %t11 to i8
  %t1 = and i8 %t2, %t7
  %t14 = load i64, ptr %p9_s, align 8
  %t15 = add i64 0, 0
  %t16 = icmp sge i64 %t14, %t15
  %t12 = zext i1 %t16 to i8
  %t0 = and i8 %t1, %t12
  %pc17 = trunc i8 %t0 to i1
  br i1 %pc17, label %body, label %done
body:
  %t20 = load i64, ptr %p0_s, align 8
  %t23 = load i64, ptr %p0_s, align 8
    %t24 = inttoptr i64 %t23 to ptr
    %t25 = getelementptr i8, ptr %t24, i64 8192
    %t26 = load i64, ptr %t25
  br label %post
post:
  br label %loop
done:
  %ret28 = load i64, ptr %result, align 8
  ret i64 %ret28
}

define i64 @host_arity_scan(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
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
  br label %loop
loop:
  %p0_l29 = load i64, ptr %p0_s, align 8
  %p1_l30 = load i64, ptr %p1_s, align 8
  %p2_l31 = load i64, ptr %p2_s, align 8
  %t2 = load i64, ptr %p2_s, align 8
  %t3 = add i64 0, 0
  %t4 = icmp sge i64 %t2, %t3
  %t0 = zext i1 %t4 to i8
  %pc5 = trunc i8 %t0 to i1
  br i1 %pc5, label %body, label %done
body:
  %t8 = load i64, ptr %p2_s, align 8
  %t11 = load i64, ptr %p0_s, align 8
  %t14 = load i64, ptr %p0_s, align 8
    %t15 = inttoptr i64 %t14 to ptr
    %t16 = getelementptr i8, ptr %t15, i64 1024
    %t17 = load i64, ptr %t16
  %t18 = icmp sge i64 %t8, %t17
  %t6 = zext i1 %t18 to i8
  %t20 = trunc i8 %t6 to i1
  br i1 %t20, label %guard.then19, label %guard.end19
  guard.then19:
  %t22 = add i64 0, 1
  %t21 = sub i64 0, %t22
  store i64 %t21, ptr %result
  br label %post
  guard.end19:
  %t29 = load i64, ptr %p0_s, align 8
  %t30 = inttoptr i64 %t29 to ptr
  %t31 = getelementptr i8, ptr %t30, i64 0
  %t33 = load i64, ptr %p2_s, align 8
  %t34 = getelementptr [64 x i64], ptr %t31, i64 0, i64 %t33
  %t35 = load i64, ptr %t34
  %t37 = load i64, ptr %p1_s, align 8
  %t38 = icmp eq i64 %t35, %t37
  %t25 = zext i1 %t38 to i8
  %t40 = trunc i8 %t25 to i1
  br i1 %t40, label %guard.then39, label %guard.end39
  guard.then39:
  %t44 = load i64, ptr %p0_s, align 8
  %t45 = inttoptr i64 %t44 to ptr
  %t46 = getelementptr i8, ptr %t45, i64 512
  %t48 = load i64, ptr %p2_s, align 8
  %t49 = getelementptr [64 x i64], ptr %t46, i64 0, i64 %t48
  %t50 = load i64, ptr %t49
  store i64 %t50, ptr %result
  br label %post
  guard.end39:
  %t55 = load i64, ptr %p0_s, align 8
  %t57 = load i64, ptr %p1_s, align 8
  %t60 = load i64, ptr %p2_s, align 8
  %t61 = add i64 0, 1
  %t58 = add nsw i64 %t60, %t61
  %t62 = inttoptr i64 %t55 to ptr
  %t53 = call i64 @host_arity_scan(ptr %state, ptr %t62, i64 %t57, i64 %t58)
  store i64 %t53, ptr %result
  br label %post
post:
  br label %loop
done:
  %ret64 = load i64, ptr %result, align 8
  ret i64 %ret64
}

define void @init_state(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #0 {
  entry:
  %ip_0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 0, ptr %ip_0, align 8
  ret void
}


define i32 @main(i32 %argc, ptr %argv) local_unnamed_addr #0 {
entry:
  store i32 %argc, ptr @__briev_argc
  store ptr %argv, ptr @__briev_argv
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

!0 = !{!"Briev"}
!1 = !{!"Int", !0}
!2 = !{!"Bit", !0}
!3 = !{!"Blob", !0}
!4 = !{!"Bool", !0}
!5 = !{!"Char", !0}
!6 = !{!"Data", !0}
!7 = !{!"Double", !0}
!8 = !{!"FP128", !0}
!9 = !{!"Float", !0}
!10 = !{!"Float32", !0}
!11 = !{!"Float64", !0}
!12 = !{!"Frame", !0}
!13 = !{!"HCallArgs", !0}
!14 = !{!"Half", !0}
!15 = !{!"HashMap", !0}
!16 = !{!"HostTable", !0}
!17 = !{!"BFloat", !0}
!18 = !{!"Int128", !0}
!19 = !{!"Int16", !0}
!20 = !{!"Int32", !0}
!21 = !{!"Int64", !0}
!22 = !{!"Int8", !0}
!23 = !{!"List", !0}
!24 = !{!"ListBuffer", !0}
!25 = !{!"PiggyBank", !0}
!26 = !{!"RingBuffer", !0}
!27 = !{!"Slice", !0}
!28 = !{!"Stack", !0}
!29 = !{!"String", !0}
!30 = !{!"UInt", !0}
!31 = !{!"UInt128", !0}
!32 = !{!"UInt16", !0}
!33 = !{!"UInt32", !0}
!34 = !{!"UInt64", !0}
!35 = !{!"UInt8", !0}
!36 = !{!"VMFrames", !0}
!37 = !{!"VMLocals", !0}
!38 = !{!"VMStack", !0}
!39 = !{!"Void", !0}
!40 = !{!"X86_FP80", !0}
!99 = distinct !{} ; StateAliasScope
