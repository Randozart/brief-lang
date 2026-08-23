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
declare i64 @briev_host_fail(i64, i64) #6
declare i64 @briev_host_print_int(i64) #6
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
@HOST_ID_LOG_INT = constant i64 1
@HOST_ID_PRINT_INT = constant i64 0
@HOST_ID_UNKNOWN_MIN = constant i64 1000
@OP_ADD = constant i64 6
@OP_AND = constant i64 11
@OP_BNOT = constant i64 28
@OP_CALL = constant i64 84
@OP_DROP = alias i64, i64* @HOST_ID_LOG_INT
@OP_DUP = constant i64 2
@OP_EQ = constant i64 17
@OP_HCALL = constant i64 113
@OP_JMP = constant i64 81
@OP_JNZ = constant i64 83
@OP_JZ = constant i64 82
@OP_LOAD = constant i64 23
@OP_LOAD_LOCAL = constant i64 49
@OP_MUL = constant i64 8
@OP_NOP = alias i64, i64* @HOST_ID_PRINT_INT
@OP_NOT = constant i64 14
@OP_OR = constant i64 12
@OP_POP_FRAME = constant i64 52
@OP_PUSH_FRAME = constant i64 51
@OP_PUSH_I16 = constant i64 80
@OP_PUSH_I32 = constant i64 112
@OP_PUSH_I64 = constant i64 144
@OP_PUSH_I8 = constant i64 48
@OP_RET = constant i64 25
@OP_SHL = constant i64 15
@OP_SHR_S = constant i64 16
@OP_STORE = constant i64 24
@OP_STORE_LOCAL = constant i64 50
@OP_SUB = constant i64 7
@OP_SWAP = constant i64 3
@OP_TRAP = constant i64 27
@OP_XOR = constant i64 13
@SYS_READ = alias i64, i64* @HOST_ID_PRINT_INT
@SYS_WRITE = alias i64, i64* @HOST_ID_LOG_INT

%StateChunk0 = type { i64, i64, i64, i64, i64, i64, i64 }
%State = type { i64, i64, i64, i64, i64, i64, i64 }
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

define i64 @read_u64(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t4 = add i64 0, 8
  %t2 = sdiv i64 %arg1, %t4
  %t5 = inttoptr i64 %ac0 to ptr
  %t6 = add i64 %t2, 0
  %t7 = getelementptr i64, ptr %t5, i64 %t6
  %t0 = load i64, ptr %t7
  %t13 = add i64 0, 8
  %t11 = sdiv i64 %arg1, %t13
  %t14 = add i64 0, 1
  %t10 = add nsw i64 %t11, %t14
  %t15 = inttoptr i64 %ac0 to ptr
  %t16 = add i64 %t10, 0
  %t17 = getelementptr i64, ptr %t15, i64 %t16
  %t8 = load i64, ptr %t17
  %t21 = add i64 0, 8
  %t19 = srem i64 %arg1, %t21
  %t22 = add i64 0, 8
  %t18 = mul nsw i64 %t19, %t22
  %t25 = add i64 0, 0
  %t26 = icmp eq i64 %t18, %t25
  %t23 = zext i1 %t26 to i8
  %t28 = trunc i8 %t23 to i1
  br i1 %t28, label %guard.then27, label %guard.end27
  guard.then27:
  ret i64 %t0
  guard.end27:
  %t33 = ashr i64 %t0, %t18
  %t39 = add i64 0, 64
  %t38 = sub nsw i64 %t39, %t18
  %t36 = shl i64 %t8, %t38
  %t32 = or i64 %t33, %t36
  ret i64 %t32
}

define i64 @fn_bc_offset(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t7 = add i64 0, 20
  %t5 = mul nsw i64 %arg2, %t7
  %t3 = add nsw i64 %arg1, %t5
  %t8 = add i64 0, 4
  %t2 = add nsw i64 %t3, %t8
  %t9 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_i64(ptr %state, ptr %t9, i64 %t2)
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
   %t6 = call i64 @briev_host_print_int(i64 %arg1)
  ret i64 %t6
  guard.end4:
  %t12 = load i64, ptr @HOST_ID_LOG_INT
  %t13 = icmp eq i64 %arg0, %t12
  %t10 = zext i1 %t13 to i8
  %t15 = trunc i8 %t10 to i1
  br i1 %t15, label %guard.then14, label %guard.end14
  guard.then14:
   %t16 = call i64 @briev_host_print_int(i64 %arg1)
  ret i64 %t16
  guard.end14:
  %t22 = load i64, ptr @HOST_ID_UNKNOWN_MIN
  %t23 = icmp sge i64 %arg0, %t22
  %t20 = zext i1 %t23 to i8
  %t25 = trunc i8 %t20 to i1
  br i1 %t25, label %guard.then24, label %guard.end24
  guard.then24:
   %t26 = call i64 @briev_host_fail(i64 %arg0, i64 %arg1)
  ret i64 %t26
  guard.end24:
   %t31 = call i64 @briev_host_fail(i64 %arg0, i64 %arg1)
  ret i64 %t31
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

define i64 @analyze_max_stack(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2, ptr %arg3, i64 %arg4) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac3 = ptrtoint ptr %arg3 to i64
  %t6 = add i64 0, 0
  %t7 = add i64 0, 0
  %t8 = inttoptr i64 %ac0 to ptr
  %t9 = inttoptr i64 %ac3 to ptr
  %t0 = call i64 @analyze_max_stack_loop(ptr %state, ptr %t8, i64 %arg1, i64 %arg2, ptr %t9, i64 %arg4, i64 %t6, i64 %t7)
  %t12 = add i64 0, 1024
  %t13 = icmp sgt i64 %t0, %t12
  %t10 = zext i1 %t13 to i8
  %t15 = trunc i8 %t10 to i1
  br i1 %t15, label %guard.then14, label %guard.end14
  guard.then14:
  %t16 = add i64 0, 1024
  ret i64 %t16
  guard.end14:
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
  guard.end4:
  %t11 = add i64 0, 2
  %t9 = mul nsw i64 %arg0, %t11
  ret i64 %t9
}

define i64 @compute_stack_slots(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2, ptr %arg3, i64 %arg4) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac3 = ptrtoint ptr %arg3 to i64
  %t6 = inttoptr i64 %ac0 to ptr
  %t7 = inttoptr i64 %ac3 to ptr
  %t0 = call i64 @analyze_max_stack(ptr %state, ptr %t6, i64 %arg1, i64 %arg2, ptr %t7, i64 %arg4)
  %t10 = add i64 0, 64
  %t11 = icmp slt i64 %t0, %t10
  %t8 = zext i1 %t11 to i8
  %t13 = trunc i8 %t8 to i1
  br i1 %t13, label %guard.then12, label %guard.end12
  guard.then12:
  %t14 = add i64 0, 64
  ret i64 %t14
  guard.end12:
  ret i64 %t0
}

define i64 @compute_locals_slots(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2, ptr %arg3, i64 %arg4) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac3 = ptrtoint ptr %arg3 to i64
  %t6 = inttoptr i64 %ac0 to ptr
  %t7 = inttoptr i64 %ac3 to ptr
  %t0 = call i64 @analyze_max_stack(ptr %state, ptr %t6, i64 %arg1, i64 %arg2, ptr %t7, i64 %arg4)
  %t10 = add i64 0, 256
  %t11 = icmp slt i64 %t0, %t10
  %t8 = zext i1 %t11 to i8
  %t13 = trunc i8 %t8 to i1
  br i1 %t13, label %guard.then12, label %guard.end12
  guard.then12:
  %t14 = add i64 0, 256
  ret i64 %t14
  guard.end12:
  %t19 = add i64 0, 2
  %t17 = mul nsw i64 %t0, %t19
  ret i64 %t17
}

define i64 @compute_frames_max(ptr noundef noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 16
  %t3 = icmp slt i64 %arg0, %t2
  %t0 = zext i1 %t3 to i8
  %t5 = trunc i8 %t0 to i1
  br i1 %t5, label %guard.then4, label %guard.end4
  guard.then4:
  %t6 = add i64 0, 16
  ret i64 %t6
  guard.end4:
  %t12 = add i64 0, 2
  %t10 = mul nsw i64 %arg0, %t12
  %t13 = add i64 0, 256
  %t14 = icmp sgt i64 %t10, %t13
  %t9 = zext i1 %t14 to i8
  %t16 = trunc i8 %t9 to i1
  br i1 %t16, label %guard.then15, label %guard.end15
  guard.then15:
  %t17 = add i64 0, 256
  ret i64 %t17
  guard.end15:
  %t22 = add i64 0, 2
  %t20 = mul nsw i64 %arg0, %t22
  ret i64 %t20
}

define i64 @tame(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, ptr %arg2, i64 %arg3, i64 %arg4) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %t2 = add i64 0, 0
  %t3 = inttoptr i64 %ac0 to ptr
  %t4 = add i64 %t2, 0
  %t5 = getelementptr i64, ptr %t3, i64 %t4
  %t0 = load i64, ptr %t5
  %t8 = add i64 0, 4294967295
  %t6 = and i64 %t0, %t8
  %t11 = add i64 0, 1380532556
  %t12 = icmp ne i64 %t6, %t11
  %t9 = zext i1 %t12 to i8
  %t14 = trunc i8 %t9 to i1
  br i1 %t14, label %guard.then13, label %guard.end13
  guard.then13:
  %t15 = add i64 0, 101
  ret i64 %t15
  guard.end13:
  %t20 = add i64 0, 4
  %t21 = inttoptr i64 %ac0 to ptr
  %t22 = add i64 %t20, 0
  %t23 = getelementptr i64, ptr %t21, i64 %t22
  %t18 = load i64, ptr %t23
  %t26 = add i64 0, 5
  %t27 = inttoptr i64 %ac0 to ptr
  %t28 = add i64 %t26, 0
  %t29 = getelementptr i64, ptr %t27, i64 %t28
  %t24 = load i64, ptr %t29
  %t32 = add i64 0, 6
  %t33 = inttoptr i64 %ac0 to ptr
  %t34 = add i64 %t32, 0
  %t35 = getelementptr i64, ptr %t33, i64 %t34
  %t30 = load i64, ptr %t35
  %t38 = add i64 0, 7
  %t39 = inttoptr i64 %ac0 to ptr
  %t40 = add i64 %t38, 0
  %t41 = getelementptr i64, ptr %t39, i64 %t40
  %t36 = load i64, ptr %t41
  %t44 = add i64 0, 8
  %t45 = inttoptr i64 %ac0 to ptr
  %t46 = add i64 %t44, 0
  %t47 = getelementptr i64, ptr %t45, i64 %t46
  %t42 = load i64, ptr %t47
  %t50 = add i64 0, 9
  %t51 = inttoptr i64 %ac0 to ptr
  %t52 = add i64 %t50, 0
  %t53 = getelementptr i64, ptr %t51, i64 %t52
  %t48 = load i64, ptr %t53
  %t56 = add i64 0, 20
  %t54 = sdiv i64 %t24, %t56
  %t58 = add nsw i64 %t30, %t36
  %t62 = icmp sgt i64 %t58, %arg1
  %t57 = zext i1 %t62 to i8
  %t64 = trunc i8 %t57 to i1
  br i1 %t64, label %guard.then63, label %guard.end63
  guard.then63:
  %t65 = add i64 0, 103
  ret i64 %t65
  guard.end63:
  %t69 = add nsw i64 %t18, %t24
  %t73 = icmp sgt i64 %t69, %arg1
  %t68 = zext i1 %t73 to i8
  %t75 = trunc i8 %t68 to i1
  br i1 %t75, label %guard.then74, label %guard.end74
  guard.then74:
  %t76 = add i64 0, 104
  ret i64 %t76
  guard.end74:
  %t81 = add i64 0, 0
  %t82 = icmp eq i64 %t54, %t81
  %t79 = zext i1 %t82 to i8
  %t84 = trunc i8 %t79 to i1
  br i1 %t84, label %guard.then83, label %guard.end83
  guard.then83:
  %t85 = add i64 0, 105
  ret i64 %t85
  guard.end83:
  %t90 = add i64 0, 1544
  %t89_p = call ptr @malloc(i64 %t90)
  %t89 = ptrtoint ptr %t89_p to i64
   %t91 = add i64 %t90, 0
  %t92 = add i64 0, 0
  %t94 = inttoptr i64 %t89 to ptr
  %t95 = getelementptr i8, ptr %t94, i64 1024
  store i64 %t92, ptr %t95
  %t98 = add i64 0, 12
  %t96 = sdiv i64 %t48, %t98
  %t101 = add i64 0, 24
  %t102 = icmp sgt i64 %t96, %t101
  %t99 = zext i1 %t102 to i8
  %t104 = trunc i8 %t99 to i1
  br i1 %t104, label %guard.then103, label %guard.end103
  guard.then103:
  %t105 = add i64 0, 106
  ret i64 %t105
  guard.end103:
  %t113 = inttoptr i64 %t89 to ptr
  %t114 = inttoptr i64 %ac0 to ptr
  %t108 = call i64 @parse_host_table(ptr %state, ptr %t113, ptr %t114, i64 %t42, i64 %t96)
  %t117 = add i64 0, 8200
  %t116_p = call ptr @malloc(i64 %t117)
  %t116 = ptrtoint ptr %t116_p to i64
   %t118 = add i64 %t117, 0
  %t121 = add i64 0, 32776
  %t120_p = call ptr @malloc(i64 %t121)
  %t120 = ptrtoint ptr %t120_p to i64
   %t122 = add i64 %t121, 0
  %t125 = add i64 0, 6152
  %t124_p = call ptr @malloc(i64 %t125)
  %t124 = ptrtoint ptr %t124_p to i64
   %t126 = add i64 %t125, 0
  %t127 = add i64 0, 0
  %t129 = inttoptr i64 %t116 to ptr
  %t130 = getelementptr i8, ptr %t129, i64 8192
  store i64 %t127, ptr %t130
  %t131 = add i64 0, 0
  %t133 = inttoptr i64 %t120 to ptr
  %t134 = getelementptr i8, ptr %t133, i64 32768
  store i64 %t131, ptr %t134
  %t135 = add i64 0, 0
  %t137 = inttoptr i64 %t124 to ptr
  %t138 = getelementptr i8, ptr %t137, i64 6144
  store i64 %t135, ptr %t138
  %t141 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t116, ptr %t141, align 8
  %t144 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 %t120, ptr %t144, align 8
  %t147 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 %t124, ptr %t147, align 8
  %t150 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 %t89, ptr %t150, align 8
  %t152 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 %t18, ptr %t152, align 8
  %t154 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store i64 %t54, ptr %t154, align 8
  %t160 = add nsw i64 %t30, %t36
  %t168 = inttoptr i64 %t116 to ptr
  %t169 = inttoptr i64 %t120 to ptr
  %t170 = inttoptr i64 %t124 to ptr
  %t171 = inttoptr i64 %ac0 to ptr
  %t172 = inttoptr i64 %ac0 to ptr
  %t173 = inttoptr i64 %t89 to ptr
  %t155 = call i64 @step(ptr %state, ptr %t168, ptr %t169, ptr %t170, ptr %t171, i64 %t160, ptr %t172, i64 %t18, i64 %t54, ptr %t173, i64 %arg4)
  ret i64 %t155
}

define i64 @buffers_stack(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #8 {
  entry:
  %t1 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t2 = load i64, ptr %t1, align 8
  ret i64 %t2
}

define i64 @buffers_locals(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #8 {
  entry:
  %t1 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t2 = load i64, ptr %t1, align 8
  ret i64 %t2
}

define i64 @buffers_frames(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #8 {
  entry:
  %t1 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t2 = load i64, ptr %t1, align 8
  ret i64 %t2
}

define i64 @buffers_ht(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #8 {
  entry:
  %t1 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t2 = load i64, ptr %t1, align 8
  ret i64 %t2
}

define i64 @rc_fn_off(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #8 {
  entry:
  %t1 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t2 = load i64, ptr %t1, align 8
  ret i64 %t2
}

define i64 @rc_fn_count(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #8 {
  entry:
  %t1 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t2 = load i64, ptr %t1, align 8
  ret i64 %t2
}

define i64 @step(ptr noundef noalias nocapture align 8 %state, ptr %arg0, ptr %arg1, ptr %arg2, ptr %arg3, i64 %arg4, ptr %arg5, i64 %arg6, i64 %arg7, ptr %arg8, i64 %arg9) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac1 = ptrtoint ptr %arg1 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %ac3 = ptrtoint ptr %arg3 to i64
  %ac5 = ptrtoint ptr %arg5 to i64
  %ac8 = ptrtoint ptr %arg8 to i64
  %t3 = add i64 0, 0
  %t4 = icmp slt i64 %arg9, %t3
  %t1 = zext i1 %t4 to i8
  %t8 = icmp sge i64 %arg9, %arg4
  %t5 = zext i1 %t8 to i8
  %t0 = or i8 %t1, %t5
  %t10 = trunc i8 %t0 to i1
  br i1 %t10, label %guard.then9, label %guard.end9
  guard.then9:
  %t12 = add i64 0, 1
  %t11 = sub i64 0, %t12
  ret i64 %t11
  guard.end9:
  %t18 = inttoptr i64 %ac3 to ptr
  %t15 = call i64 @read_u8(ptr %state, ptr %t18, i64 %arg9)
  %t30 = add i64 0, 0
  %t31 = inttoptr i64 %ac0 to ptr
  %t32 = inttoptr i64 %ac1 to ptr
  %t33 = inttoptr i64 %ac2 to ptr
  %t34 = inttoptr i64 %ac3 to ptr
  %t35 = inttoptr i64 %ac5 to ptr
  %t36 = inttoptr i64 %ac8 to ptr
  %t19 = call i64 @exec_op(ptr %state, ptr %t31, ptr %t32, ptr %t33, ptr %t34, ptr %t35, i64 %arg6, i64 %arg7, ptr %t36, i64 %t15, i64 %arg9, i64 %t30)
  ret i64 %t19
}

define i64 @parse_host_table(ptr noundef noalias nocapture align 8 %state, ptr %arg0, ptr %arg1, i64 %arg2, i64 %arg3) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac1 = ptrtoint ptr %arg1 to i64
  %t2 = add i64 0, 1
  %t3 = icmp sge i64 %arg3, %t2
  %t0 = zext i1 %t3 to i8
  %t5 = trunc i8 %t0 to i1
  br i1 %t5, label %guard.then4, label %guard.end4
  guard.then4:
  %t10 = add i64 0, 0
  %t11 = inttoptr i64 %ac0 to ptr
  %t12 = inttoptr i64 %ac1 to ptr
  %t6 = call i64 @parse_host_entry(ptr %state, ptr %t11, ptr %t12, i64 %arg2, i64 %t10)
  br label %guard.end4
  guard.end4:
  %t16 = add i64 0, 2
  %t17 = icmp sge i64 %arg3, %t16
  %t14 = zext i1 %t17 to i8
  %t19 = trunc i8 %t14 to i1
  br i1 %t19, label %guard.then18, label %guard.end18
  guard.then18:
  %t24 = add i64 0, 1
  %t25 = inttoptr i64 %ac0 to ptr
  %t26 = inttoptr i64 %ac1 to ptr
  %t20 = call i64 @parse_host_entry(ptr %state, ptr %t25, ptr %t26, i64 %arg2, i64 %t24)
  br label %guard.end18
  guard.end18:
  %t30 = add i64 0, 3
  %t31 = icmp sge i64 %arg3, %t30
  %t28 = zext i1 %t31 to i8
  %t33 = trunc i8 %t28 to i1
  br i1 %t33, label %guard.then32, label %guard.end32
  guard.then32:
  %t38 = add i64 0, 2
  %t39 = inttoptr i64 %ac0 to ptr
  %t40 = inttoptr i64 %ac1 to ptr
  %t34 = call i64 @parse_host_entry(ptr %state, ptr %t39, ptr %t40, i64 %arg2, i64 %t38)
  br label %guard.end32
  guard.end32:
  %t44 = add i64 0, 4
  %t45 = icmp sge i64 %arg3, %t44
  %t42 = zext i1 %t45 to i8
  %t47 = trunc i8 %t42 to i1
  br i1 %t47, label %guard.then46, label %guard.end46
  guard.then46:
  %t52 = add i64 0, 3
  %t53 = inttoptr i64 %ac0 to ptr
  %t54 = inttoptr i64 %ac1 to ptr
  %t48 = call i64 @parse_host_entry(ptr %state, ptr %t53, ptr %t54, i64 %arg2, i64 %t52)
  br label %guard.end46
  guard.end46:
  %t58 = add i64 0, 5
  %t59 = icmp sge i64 %arg3, %t58
  %t56 = zext i1 %t59 to i8
  %t61 = trunc i8 %t56 to i1
  br i1 %t61, label %guard.then60, label %guard.end60
  guard.then60:
  %t66 = add i64 0, 4
  %t67 = inttoptr i64 %ac0 to ptr
  %t68 = inttoptr i64 %ac1 to ptr
  %t62 = call i64 @parse_host_entry(ptr %state, ptr %t67, ptr %t68, i64 %arg2, i64 %t66)
  br label %guard.end60
  guard.end60:
  %t72 = add i64 0, 6
  %t73 = icmp sge i64 %arg3, %t72
  %t70 = zext i1 %t73 to i8
  %t75 = trunc i8 %t70 to i1
  br i1 %t75, label %guard.then74, label %guard.end74
  guard.then74:
  %t80 = add i64 0, 5
  %t81 = inttoptr i64 %ac0 to ptr
  %t82 = inttoptr i64 %ac1 to ptr
  %t76 = call i64 @parse_host_entry(ptr %state, ptr %t81, ptr %t82, i64 %arg2, i64 %t80)
  br label %guard.end74
  guard.end74:
  %t86 = add i64 0, 7
  %t87 = icmp sge i64 %arg3, %t86
  %t84 = zext i1 %t87 to i8
  %t89 = trunc i8 %t84 to i1
  br i1 %t89, label %guard.then88, label %guard.end88
  guard.then88:
  %t94 = add i64 0, 6
  %t95 = inttoptr i64 %ac0 to ptr
  %t96 = inttoptr i64 %ac1 to ptr
  %t90 = call i64 @parse_host_entry(ptr %state, ptr %t95, ptr %t96, i64 %arg2, i64 %t94)
  br label %guard.end88
  guard.end88:
  %t100 = add i64 0, 8
  %t101 = icmp sge i64 %arg3, %t100
  %t98 = zext i1 %t101 to i8
  %t103 = trunc i8 %t98 to i1
  br i1 %t103, label %guard.then102, label %guard.end102
  guard.then102:
  %t108 = add i64 0, 7
  %t109 = inttoptr i64 %ac0 to ptr
  %t110 = inttoptr i64 %ac1 to ptr
  %t104 = call i64 @parse_host_entry(ptr %state, ptr %t109, ptr %t110, i64 %arg2, i64 %t108)
  br label %guard.end102
  guard.end102:
  %t114 = add i64 0, 9
  %t115 = icmp sge i64 %arg3, %t114
  %t112 = zext i1 %t115 to i8
  %t117 = trunc i8 %t112 to i1
  br i1 %t117, label %guard.then116, label %guard.end116
  guard.then116:
  %t122 = add i64 0, 8
  %t123 = inttoptr i64 %ac0 to ptr
  %t124 = inttoptr i64 %ac1 to ptr
  %t118 = call i64 @parse_host_entry(ptr %state, ptr %t123, ptr %t124, i64 %arg2, i64 %t122)
  br label %guard.end116
  guard.end116:
  %t128 = add i64 0, 10
  %t129 = icmp sge i64 %arg3, %t128
  %t126 = zext i1 %t129 to i8
  %t131 = trunc i8 %t126 to i1
  br i1 %t131, label %guard.then130, label %guard.end130
  guard.then130:
  %t136 = add i64 0, 9
  %t137 = inttoptr i64 %ac0 to ptr
  %t138 = inttoptr i64 %ac1 to ptr
  %t132 = call i64 @parse_host_entry(ptr %state, ptr %t137, ptr %t138, i64 %arg2, i64 %t136)
  br label %guard.end130
  guard.end130:
  %t142 = add i64 0, 11
  %t143 = icmp sge i64 %arg3, %t142
  %t140 = zext i1 %t143 to i8
  %t145 = trunc i8 %t140 to i1
  br i1 %t145, label %guard.then144, label %guard.end144
  guard.then144:
  %t150 = add i64 0, 10
  %t151 = inttoptr i64 %ac0 to ptr
  %t152 = inttoptr i64 %ac1 to ptr
  %t146 = call i64 @parse_host_entry(ptr %state, ptr %t151, ptr %t152, i64 %arg2, i64 %t150)
  br label %guard.end144
  guard.end144:
  %t156 = add i64 0, 12
  %t157 = icmp sge i64 %arg3, %t156
  %t154 = zext i1 %t157 to i8
  %t159 = trunc i8 %t154 to i1
  br i1 %t159, label %guard.then158, label %guard.end158
  guard.then158:
  %t164 = add i64 0, 11
  %t165 = inttoptr i64 %ac0 to ptr
  %t166 = inttoptr i64 %ac1 to ptr
  %t160 = call i64 @parse_host_entry(ptr %state, ptr %t165, ptr %t166, i64 %arg2, i64 %t164)
  br label %guard.end158
  guard.end158:
  %t170 = add i64 0, 13
  %t171 = icmp sge i64 %arg3, %t170
  %t168 = zext i1 %t171 to i8
  %t173 = trunc i8 %t168 to i1
  br i1 %t173, label %guard.then172, label %guard.end172
  guard.then172:
  %t178 = add i64 0, 12
  %t179 = inttoptr i64 %ac0 to ptr
  %t180 = inttoptr i64 %ac1 to ptr
  %t174 = call i64 @parse_host_entry(ptr %state, ptr %t179, ptr %t180, i64 %arg2, i64 %t178)
  br label %guard.end172
  guard.end172:
  %t184 = add i64 0, 14
  %t185 = icmp sge i64 %arg3, %t184
  %t182 = zext i1 %t185 to i8
  %t187 = trunc i8 %t182 to i1
  br i1 %t187, label %guard.then186, label %guard.end186
  guard.then186:
  %t192 = add i64 0, 13
  %t193 = inttoptr i64 %ac0 to ptr
  %t194 = inttoptr i64 %ac1 to ptr
  %t188 = call i64 @parse_host_entry(ptr %state, ptr %t193, ptr %t194, i64 %arg2, i64 %t192)
  br label %guard.end186
  guard.end186:
  %t198 = add i64 0, 15
  %t199 = icmp sge i64 %arg3, %t198
  %t196 = zext i1 %t199 to i8
  %t201 = trunc i8 %t196 to i1
  br i1 %t201, label %guard.then200, label %guard.end200
  guard.then200:
  %t206 = add i64 0, 14
  %t207 = inttoptr i64 %ac0 to ptr
  %t208 = inttoptr i64 %ac1 to ptr
  %t202 = call i64 @parse_host_entry(ptr %state, ptr %t207, ptr %t208, i64 %arg2, i64 %t206)
  br label %guard.end200
  guard.end200:
  %t212 = add i64 0, 16
  %t213 = icmp sge i64 %arg3, %t212
  %t210 = zext i1 %t213 to i8
  %t215 = trunc i8 %t210 to i1
  br i1 %t215, label %guard.then214, label %guard.end214
  guard.then214:
  %t220 = add i64 0, 15
  %t221 = inttoptr i64 %ac0 to ptr
  %t222 = inttoptr i64 %ac1 to ptr
  %t216 = call i64 @parse_host_entry(ptr %state, ptr %t221, ptr %t222, i64 %arg2, i64 %t220)
  br label %guard.end214
  guard.end214:
  %t226 = add i64 0, 17
  %t227 = icmp sge i64 %arg3, %t226
  %t224 = zext i1 %t227 to i8
  %t229 = trunc i8 %t224 to i1
  br i1 %t229, label %guard.then228, label %guard.end228
  guard.then228:
  %t234 = add i64 0, 16
  %t235 = inttoptr i64 %ac0 to ptr
  %t236 = inttoptr i64 %ac1 to ptr
  %t230 = call i64 @parse_host_entry(ptr %state, ptr %t235, ptr %t236, i64 %arg2, i64 %t234)
  br label %guard.end228
  guard.end228:
  %t240 = add i64 0, 18
  %t241 = icmp sge i64 %arg3, %t240
  %t238 = zext i1 %t241 to i8
  %t243 = trunc i8 %t238 to i1
  br i1 %t243, label %guard.then242, label %guard.end242
  guard.then242:
  %t248 = add i64 0, 17
  %t249 = inttoptr i64 %ac0 to ptr
  %t250 = inttoptr i64 %ac1 to ptr
  %t244 = call i64 @parse_host_entry(ptr %state, ptr %t249, ptr %t250, i64 %arg2, i64 %t248)
  br label %guard.end242
  guard.end242:
  %t254 = add i64 0, 19
  %t255 = icmp sge i64 %arg3, %t254
  %t252 = zext i1 %t255 to i8
  %t257 = trunc i8 %t252 to i1
  br i1 %t257, label %guard.then256, label %guard.end256
  guard.then256:
  %t262 = add i64 0, 18
  %t263 = inttoptr i64 %ac0 to ptr
  %t264 = inttoptr i64 %ac1 to ptr
  %t258 = call i64 @parse_host_entry(ptr %state, ptr %t263, ptr %t264, i64 %arg2, i64 %t262)
  br label %guard.end256
  guard.end256:
  %t268 = add i64 0, 20
  %t269 = icmp sge i64 %arg3, %t268
  %t266 = zext i1 %t269 to i8
  %t271 = trunc i8 %t266 to i1
  br i1 %t271, label %guard.then270, label %guard.end270
  guard.then270:
  %t276 = add i64 0, 19
  %t277 = inttoptr i64 %ac0 to ptr
  %t278 = inttoptr i64 %ac1 to ptr
  %t272 = call i64 @parse_host_entry(ptr %state, ptr %t277, ptr %t278, i64 %arg2, i64 %t276)
  br label %guard.end270
  guard.end270:
  %t282 = add i64 0, 21
  %t283 = icmp sge i64 %arg3, %t282
  %t280 = zext i1 %t283 to i8
  %t285 = trunc i8 %t280 to i1
  br i1 %t285, label %guard.then284, label %guard.end284
  guard.then284:
  %t290 = add i64 0, 20
  %t291 = inttoptr i64 %ac0 to ptr
  %t292 = inttoptr i64 %ac1 to ptr
  %t286 = call i64 @parse_host_entry(ptr %state, ptr %t291, ptr %t292, i64 %arg2, i64 %t290)
  br label %guard.end284
  guard.end284:
  %t296 = add i64 0, 22
  %t297 = icmp sge i64 %arg3, %t296
  %t294 = zext i1 %t297 to i8
  %t299 = trunc i8 %t294 to i1
  br i1 %t299, label %guard.then298, label %guard.end298
  guard.then298:
  %t304 = add i64 0, 21
  %t305 = inttoptr i64 %ac0 to ptr
  %t306 = inttoptr i64 %ac1 to ptr
  %t300 = call i64 @parse_host_entry(ptr %state, ptr %t305, ptr %t306, i64 %arg2, i64 %t304)
  br label %guard.end298
  guard.end298:
  %t310 = add i64 0, 23
  %t311 = icmp sge i64 %arg3, %t310
  %t308 = zext i1 %t311 to i8
  %t313 = trunc i8 %t308 to i1
  br i1 %t313, label %guard.then312, label %guard.end312
  guard.then312:
  %t318 = add i64 0, 22
  %t319 = inttoptr i64 %ac0 to ptr
  %t320 = inttoptr i64 %ac1 to ptr
  %t314 = call i64 @parse_host_entry(ptr %state, ptr %t319, ptr %t320, i64 %arg2, i64 %t318)
  br label %guard.end312
  guard.end312:
  %t324 = add i64 0, 24
  %t325 = icmp sge i64 %arg3, %t324
  %t322 = zext i1 %t325 to i8
  %t327 = trunc i8 %t322 to i1
  br i1 %t327, label %guard.then326, label %guard.end326
  guard.then326:
  %t332 = add i64 0, 23
  %t333 = inttoptr i64 %ac0 to ptr
  %t334 = inttoptr i64 %ac1 to ptr
  %t328 = call i64 @parse_host_entry(ptr %state, ptr %t333, ptr %t334, i64 %arg2, i64 %t332)
  br label %guard.end326
  guard.end326:
  %t338 = inttoptr i64 %ac0 to ptr
  %t339 = getelementptr i8, ptr %t338, i64 1024
  store i64 %arg3, ptr %t339
  ret i64 %arg3
}

define i64 @parse_host_entry(ptr noundef noalias nocapture align 8 %state, ptr %arg0, ptr %arg1, i64 %arg2, i64 %arg3) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac1 = ptrtoint ptr %arg1 to i64
  %t4 = add i64 0, 12
  %t2 = mul nsw i64 %arg3, %t4
  %t0 = add nsw i64 %arg2, %t2
  %t9 = add i64 0, 4
  %t7 = add nsw i64 %t0, %t9
  %t10 = inttoptr i64 %ac1 to ptr
  %t5 = call i64 @read_u32(ptr %state, ptr %t10, i64 %t7)
  %t13 = inttoptr i64 %ac0 to ptr
  %t14 = getelementptr i8, ptr %t13, i64 0
  %t16 = getelementptr [64 x i64], ptr %t14, i64 0, i64 %arg3
  store i64 %t5, ptr %t16
  %t21 = add i64 0, 8
  %t19 = add nsw i64 %t0, %t21
  %t22 = inttoptr i64 %ac1 to ptr
  %t17 = call i64 @read_u32(ptr %state, ptr %t22, i64 %t19)
  %t25 = inttoptr i64 %ac0 to ptr
  %t26 = getelementptr i8, ptr %t25, i64 512
  %t28 = getelementptr [64 x i64], ptr %t26, i64 0, i64 %arg3
  store i64 %t17, ptr %t28
  %t29 = add i64 0, 0
  ret i64 %t29
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
  %p0_l31 = load i64, ptr %p0_s, align 8
  %p1_l32 = load i64, ptr %p1_s, align 8
  %p2_l33 = load i64, ptr %p2_s, align 8
  %p3_l34 = load i64, ptr %p3_s, align 8
  %p4_l35 = load i64, ptr %p4_s, align 8
  %p5_l36 = load i64, ptr %p5_s, align 8
  %p6_l37 = load i64, ptr %p6_s, align 8
  %p7_l38 = load i64, ptr %p7_s, align 8
  %p8_l39 = load i64, ptr %p8_s, align 8
  %p9_l40 = load i64, ptr %p9_s, align 8
  %p10_l41 = load i64, ptr %p10_s, align 8
  %p11_l42 = load i64, ptr %p11_s, align 8
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
  %p0_l65 = load i64, ptr %p0_s, align 8
  %p1_l66 = load i64, ptr %p1_s, align 8
  %p2_l67 = load i64, ptr %p2_s, align 8
  %p3_l68 = load i64, ptr %p3_s, align 8
  %p4_l69 = load i64, ptr %p4_s, align 8
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

define i64 @analyze_max_stack_loop(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2, ptr %arg3, i64 %arg4, i64 %arg5, i64 %arg6) local_unnamed_addr #8 {
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
  %ac3 = ptrtoint ptr %arg3 to i64
  %p3_s = alloca i64, align 8
  store i64 %ac3, ptr %p3_s, align 8
  %p4_s = alloca i64, align 8
  store i64 %arg4, ptr %p4_s, align 8
  %p5_s = alloca i64, align 8
  store i64 %arg5, ptr %p5_s, align 8
  %p6_s = alloca i64, align 8
  store i64 %arg6, ptr %p6_s, align 8
  br label %loop
loop:
  %p0_l48 = load i64, ptr %p0_s, align 8
  %p1_l49 = load i64, ptr %p1_s, align 8
  %p2_l50 = load i64, ptr %p2_s, align 8
  %p3_l51 = load i64, ptr %p3_s, align 8
  %p4_l52 = load i64, ptr %p4_s, align 8
  %p5_l53 = load i64, ptr %p5_s, align 8
  %p6_l54 = load i64, ptr %p6_s, align 8
  %t2 = load i64, ptr %p5_s, align 8
  %t4 = load i64, ptr %p2_s, align 8
  %t5 = icmp slt i64 %t2, %t4
  %t0 = zext i1 %t5 to i8
  %pc6 = trunc i8 %t0 to i1
  br i1 %pc6, label %body, label %done
body:
  %t9 = load i64, ptr %p0_s, align 8
  %t11 = load i64, ptr %p1_s, align 8
  %t13 = load i64, ptr %p5_s, align 8
  %t14 = inttoptr i64 %t9 to ptr
  %t7 = call i64 @fn_bc_offset(ptr %state, ptr %t14, i64 %t11, i64 %t13)
  %t17 = load i64, ptr %p0_s, align 8
  %t19 = load i64, ptr %p1_s, align 8
  %t21 = load i64, ptr %p5_s, align 8
  %t22 = inttoptr i64 %t17 to ptr
  %t15 = call i64 @fn_bc_len(ptr %state, ptr %t22, i64 %t19, i64 %t21)
  %t25 = load i64, ptr %p0_s, align 8
  %t27 = load i64, ptr %p1_s, align 8
  %t29 = load i64, ptr %p5_s, align 8
  %t30 = inttoptr i64 %t25 to ptr
  %t23 = call i64 @fn_local_count(ptr %state, ptr %t30, i64 %t27, i64 %t29)
  %t33 = load i64, ptr %p3_s, align 8
  %t36 = inttoptr i64 %t33 to ptr
  %t31 = call i64 @analyze_fn_stack(ptr %state, ptr %t36, i64 %t7, i64 %t15)
  %t37 = add nsw i64 %t31, %t23
  %t43 = load i64, ptr %p6_s, align 8
  %t44 = icmp sgt i64 %t37, %t43
  %t40 = zext i1 %t44 to i8
  %t46 = trunc i8 %t40 to i1
  br i1 %t46, label %guard.then45, label %guard.end45
  guard.then45:
  store i64 %t37, ptr %p6_s
  br label %guard.end45
  guard.end45:
  %t51 = load i64, ptr %p5_s, align 8
  %t52 = add i64 0, 1
  %t49 = add nsw i64 %t51, %t52
  store i64 %t49, ptr %p5_s
  %t54 = load i64, ptr %p6_s, align 8
  store i64 %t54, ptr %result
  br label %post
post:
  br label %loop
done:
  %ret56 = load i64, ptr %result, align 8
  ret i64 %ret56
}

define void @init_state(ptr noundef noalias nocapture align 8 %state) local_unnamed_addr #0 {
  entry:
  %ip_0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 0, ptr %ip_0, align 8
  %ip_1 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 0, ptr %ip_1, align 8
  %ip_2 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 0, ptr %ip_2, align 8
  %ip_3 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 0, ptr %ip_3, align 8
  %ip_4 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 0, ptr %ip_4, align 8
  %ip_5 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store i64 0, ptr %ip_5, align 8
  %ip_6 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 0, ptr %ip_6, align 8
  ret void
}


define i32 @main(i32 %argc, ptr %argv) local_unnamed_addr #0 {
entry:
  store i32 %argc, ptr @__briev_argc
  store ptr %argv, ptr @__briev_argv
  %state = alloca %State, align 8
  %t0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 0, ptr %t0, align 8
  %t1 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 0, ptr %t1, align 8
  %t2 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 0, ptr %t2, align 8
  %t3 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 0, ptr %t3, align 8
  %t4 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 0, ptr %t4, align 8
  %t5 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store i64 0, ptr %t5, align 8
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 0, ptr %t6, align 8
  %sa7 = alloca i64, align 8
  %sa8 = alloca i64, align 8
  %sa9 = alloca i64, align 8
  %sa10 = alloca i64, align 8
  %sa11 = alloca i64, align 8
  %sa12 = alloca i64, align 8
  %sa13 = alloca i64, align 8
  %any_active_14 = alloca i64, align 8
  br label %.ss_main_loop
.ss_main_loop:
  store i64 0, ptr %any_active_14
  %t15 = load i64, ptr %any_active_14
  %t16 = icmp eq i64 %t15, 0
  br i1 %t16, label %.end, label %.ss_main_loop
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
