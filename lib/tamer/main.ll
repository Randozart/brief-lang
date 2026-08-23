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
declare i64 @briev_host_arity_of(i64) #6
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
  %t4 = inttoptr i64 %ac0 to ptr
  %t1 = call i64 @read_u8(ptr %state, ptr %t4, i64 %arg1)
  %t10 = add i64 0, 1
  %t8 = add nsw i64 %arg1, %t10
  %t11 = inttoptr i64 %ac0 to ptr
  %t6 = call i64 @read_u8(ptr %state, ptr %t11, i64 %t8)
  %t12 = add i64 0, 8
  %t5 = shl i64 %t6, %t12
  %t0 = or i64 %t1, %t5
  ret i64 %t0
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
  %t6 = inttoptr i64 %ac0 to ptr
  %t3 = call i64 @read_u8(ptr %state, ptr %t6, i64 %arg1)
  %t12 = add i64 0, 1
  %t10 = add nsw i64 %arg1, %t12
  %t13 = inttoptr i64 %ac0 to ptr
  %t8 = call i64 @read_u8(ptr %state, ptr %t13, i64 %t10)
  %t14 = add i64 0, 8
  %t7 = shl i64 %t8, %t14
  %t2 = or i64 %t3, %t7
  %t20 = add i64 0, 2
  %t18 = add nsw i64 %arg1, %t20
  %t21 = inttoptr i64 %ac0 to ptr
  %t16 = call i64 @read_u8(ptr %state, ptr %t21, i64 %t18)
  %t22 = add i64 0, 16
  %t15 = shl i64 %t16, %t22
  %t1 = or i64 %t2, %t15
  %t28 = add i64 0, 3
  %t26 = add nsw i64 %arg1, %t28
  %t29 = inttoptr i64 %ac0 to ptr
  %t24 = call i64 @read_u8(ptr %state, ptr %t29, i64 %t26)
  %t30 = add i64 0, 24
  %t23 = shl i64 %t24, %t30
  %t0 = or i64 %t1, %t23
  ret i64 %t0
}

define i64 @read_i32(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t3 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u32(ptr %state, ptr %t3, i64 %arg1)
  %t6 = add i64 0, 2147483648
  %t7 = icmp sge i64 %t0, %t6
  %t4 = zext i1 %t7 to i8
  %t9 = trunc i8 %t4 to i1
  br i1 %t9, label %guard.then8, label %guard.end8
  guard.then8:
  %t12 = add i64 0, 4294967296
  %t10 = sub nsw i64 %t0, %t12
  ret i64 %t10
  guard.end8:
  ret i64 %t0
}

define i64 @read_i64(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t3 = inttoptr i64 %ac0 to ptr
  %t0 = call i64 @read_u32(ptr %state, ptr %t3, i64 %arg1)
  %t8 = add i64 0, 4
  %t6 = add nsw i64 %arg1, %t8
  %t9 = inttoptr i64 %ac0 to ptr
  %t4 = call i64 @read_u32(ptr %state, ptr %t9, i64 %t6)
  %t14 = add i64 0, 32
  %t12 = shl i64 %t4, %t14
  %t10 = or i64 %t0, %t12
  ret i64 %t10
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

define i64 @exec_op(ptr noundef noalias nocapture align 8 %state, ptr %arg0, ptr %arg1, ptr %arg2, ptr %arg3, ptr %arg4, i64 %arg5, i64 %arg6, i64 %arg7, i64 %arg8, i64 %arg9) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac1 = ptrtoint ptr %arg1 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %ac3 = ptrtoint ptr %arg3 to i64
  %ac4 = ptrtoint ptr %arg4 to i64
    %t4 = inttoptr i64 %ac0 to ptr
    %t5 = getelementptr i8, ptr %t4, i64 8192
    %t6 = load i64, ptr %t5
  %t9 = add i64 0, 0
  %t10 = icmp eq i64 %arg7, %t9
  br i1 %t10, label %.smt_body_8_0, label %.smt_next_8_0
.smt_next_8_0:
  %t11 = add i64 0, 1
  %t12 = icmp eq i64 %arg7, %t11
  br i1 %t12, label %.smt_body_8_1, label %.smt_next_8_1
.smt_next_8_1:
  %t13 = add i64 0, 2
  %t14 = icmp eq i64 %arg7, %t13
  br i1 %t14, label %.smt_body_8_2, label %.smt_next_8_2
.smt_next_8_2:
  %t15 = add i64 0, 3
  %t16 = icmp eq i64 %arg7, %t15
  br i1 %t16, label %.smt_body_8_3, label %.smt_next_8_3
.smt_next_8_3:
  %t17 = add i64 0, 6
  %t18 = icmp eq i64 %arg7, %t17
  br i1 %t18, label %.smt_body_8_4, label %.smt_next_8_4
.smt_next_8_4:
  %t19 = add i64 0, 7
  %t20 = icmp eq i64 %arg7, %t19
  br i1 %t20, label %.smt_body_8_5, label %.smt_next_8_5
.smt_next_8_5:
  %t21 = add i64 0, 8
  %t22 = icmp eq i64 %arg7, %t21
  br i1 %t22, label %.smt_body_8_6, label %.smt_next_8_6
.smt_next_8_6:
  %t23 = add i64 0, 11
  %t24 = icmp eq i64 %arg7, %t23
  br i1 %t24, label %.smt_body_8_7, label %.smt_next_8_7
.smt_next_8_7:
  %t25 = add i64 0, 12
  %t26 = icmp eq i64 %arg7, %t25
  br i1 %t26, label %.smt_body_8_8, label %.smt_next_8_8
.smt_next_8_8:
  %t27 = add i64 0, 13
  %t28 = icmp eq i64 %arg7, %t27
  br i1 %t28, label %.smt_body_8_9, label %.smt_next_8_9
.smt_next_8_9:
  %t29 = add i64 0, 14
  %t30 = icmp eq i64 %arg7, %t29
  br i1 %t30, label %.smt_body_8_10, label %.smt_next_8_10
.smt_next_8_10:
  %t31 = add i64 0, 15
  %t32 = icmp eq i64 %arg7, %t31
  br i1 %t32, label %.smt_body_8_11, label %.smt_next_8_11
.smt_next_8_11:
  %t33 = add i64 0, 16
  %t34 = icmp eq i64 %arg7, %t33
  br i1 %t34, label %.smt_body_8_12, label %.smt_next_8_12
.smt_next_8_12:
  %t35 = add i64 0, 17
  %t36 = icmp eq i64 %arg7, %t35
  br i1 %t36, label %.smt_body_8_13, label %.smt_next_8_13
.smt_next_8_13:
  %t37 = add i64 0, 23
  %t38 = icmp eq i64 %arg7, %t37
  br i1 %t38, label %.smt_body_8_14, label %.smt_next_8_14
.smt_next_8_14:
  %t39 = add i64 0, 24
  %t40 = icmp eq i64 %arg7, %t39
  br i1 %t40, label %.smt_body_8_15, label %.smt_next_8_15
.smt_next_8_15:
  %t41 = add i64 0, 25
  %t42 = icmp eq i64 %arg7, %t41
  br i1 %t42, label %.smt_body_8_16, label %.smt_next_8_16
.smt_next_8_16:
  %t43 = add i64 0, 27
  %t44 = icmp eq i64 %arg7, %t43
  br i1 %t44, label %.smt_body_8_17, label %.smt_next_8_17
.smt_next_8_17:
  %t45 = add i64 0, 48
  %t46 = icmp eq i64 %arg7, %t45
  br i1 %t46, label %.smt_body_8_18, label %.smt_next_8_18
.smt_next_8_18:
  %t47 = add i64 0, 49
  %t48 = icmp eq i64 %arg7, %t47
  br i1 %t48, label %.smt_body_8_19, label %.smt_next_8_19
.smt_next_8_19:
  %t49 = add i64 0, 50
  %t50 = icmp eq i64 %arg7, %t49
  br i1 %t50, label %.smt_body_8_20, label %.smt_next_8_20
.smt_next_8_20:
  %t51 = add i64 0, 51
  %t52 = icmp eq i64 %arg7, %t51
  br i1 %t52, label %.smt_body_8_21, label %.smt_next_8_21
.smt_next_8_21:
  %t53 = add i64 0, 52
  %t54 = icmp eq i64 %arg7, %t53
  br i1 %t54, label %.smt_body_8_22, label %.smt_next_8_22
.smt_next_8_22:
  %t55 = add i64 0, 80
  %t56 = icmp eq i64 %arg7, %t55
  br i1 %t56, label %.smt_body_8_23, label %.smt_next_8_23
.smt_next_8_23:
  %t57 = add i64 0, 81
  %t58 = icmp eq i64 %arg7, %t57
  br i1 %t58, label %.smt_body_8_24, label %.smt_next_8_24
.smt_next_8_24:
  %t59 = add i64 0, 82
  %t60 = icmp eq i64 %arg7, %t59
  br i1 %t60, label %.smt_body_8_25, label %.smt_next_8_25
.smt_next_8_25:
  %t61 = add i64 0, 83
  %t62 = icmp eq i64 %arg7, %t61
  br i1 %t62, label %.smt_body_8_26, label %.smt_next_8_26
.smt_next_8_26:
  %t63 = add i64 0, 84
  %t64 = icmp eq i64 %arg7, %t63
  br i1 %t64, label %.smt_body_8_27, label %.smt_next_8_27
.smt_next_8_27:
  %t65 = add i64 0, 112
  %t66 = icmp eq i64 %arg7, %t65
  br i1 %t66, label %.smt_body_8_28, label %.smt_next_8_28
.smt_next_8_28:
  %t67 = add i64 0, 113
  %t68 = icmp eq i64 %arg7, %t67
  br i1 %t68, label %.smt_body_8_29, label %.smt_next_8_29
.smt_next_8_29:
  %t69 = add i64 0, 144
  %t70 = icmp eq i64 %arg7, %t69
  br i1 %t70, label %.smt_body_8_30, label %.smt_next_8_30
.smt_next_8_30:
  %t71 = add i64 0, 28
  %t72 = icmp eq i64 %arg7, %t71
  br i1 %t72, label %.smt_body_8_31, label %.smt_next_8_31
.smt_next_8_31:
  %t73 = icmp eq i64 0, 0
  br i1 %t73, label %.smt_body_8_32, label %.smt_next_8_32
.smt_next_8_32:
  br label %.smt_end_8
.smt_body_8_0:
  %t76 = add i64 0, 1
  %t74 = add nsw i64 %arg8, %t76
  ret i64 %t74
.smt_body_8_1:
  %t80 = add i64 0, 0
  %t81 = icmp sgt i64 %t6, %t80
  %t78 = zext i1 %t81 to i8
  %t83 = trunc i8 %t78 to i1
  br i1 %t83, label %guard.then82, label %guard.end82
  guard.then82:
  %t86 = add i64 0, 1
  %t84 = sub nsw i64 %t6, %t86
  %t88 = inttoptr i64 %ac0 to ptr
  %t89 = getelementptr i8, ptr %t88, i64 8192
  store i64 %t84, ptr %t89
  br label %guard.end82
  guard.end82:
  %t93 = add i64 0, 1
  %t91 = add nsw i64 %arg8, %t93
  ret i64 %t91
.smt_body_8_2:
  %t97 = add i64 0, 0
  %t98 = icmp sgt i64 %t6, %t97
  %t95 = zext i1 %t98 to i8
  %t100 = trunc i8 %t95 to i1
  br i1 %t100, label %guard.then99, label %guard.end99
  guard.then99:
  %t104 = inttoptr i64 %ac0 to ptr
  %t105 = getelementptr i8, ptr %t104, i64 0
  %t108 = add i64 0, 1
  %t106 = sub nsw i64 %t6, %t108
  %t109 = getelementptr [1024 x i64], ptr %t105, i64 0, i64 %t106
  %t110 = load i64, ptr %t109
  %t113 = inttoptr i64 %ac0 to ptr
  %t114 = getelementptr i8, ptr %t113, i64 0
  %t116 = getelementptr [1024 x i64], ptr %t114, i64 0, i64 %t6
  store i64 %t110, ptr %t116
  %t119 = add i64 0, 1
  %t117 = add nsw i64 %t6, %t119
  %t121 = inttoptr i64 %ac0 to ptr
  %t122 = getelementptr i8, ptr %t121, i64 8192
  store i64 %t117, ptr %t122
  br label %guard.end99
  guard.end99:
  %t126 = add i64 0, 1
  %t124 = add nsw i64 %arg8, %t126
  ret i64 %t124
.smt_body_8_3:
  %t130 = add i64 0, 2
  %t131 = icmp sge i64 %t6, %t130
  %t128 = zext i1 %t131 to i8
  %t133 = trunc i8 %t128 to i1
  br i1 %t133, label %guard.then132, label %guard.end132
  guard.then132:
  %t137 = inttoptr i64 %ac0 to ptr
  %t138 = getelementptr i8, ptr %t137, i64 0
  %t141 = add i64 0, 1
  %t139 = sub nsw i64 %t6, %t141
  %t142 = getelementptr [1024 x i64], ptr %t138, i64 0, i64 %t139
  %t143 = load i64, ptr %t142
  %t147 = inttoptr i64 %ac0 to ptr
  %t148 = getelementptr i8, ptr %t147, i64 0
  %t151 = add i64 0, 2
  %t149 = sub nsw i64 %t6, %t151
  %t152 = getelementptr [1024 x i64], ptr %t148, i64 0, i64 %t149
  %t153 = load i64, ptr %t152
  %t157 = inttoptr i64 %ac0 to ptr
  %t158 = getelementptr i8, ptr %t157, i64 0
  %t161 = add i64 0, 2
  %t159 = sub nsw i64 %t6, %t161
  %t162 = getelementptr [1024 x i64], ptr %t158, i64 0, i64 %t159
  store i64 %t143, ptr %t162
  %t166 = inttoptr i64 %ac0 to ptr
  %t167 = getelementptr i8, ptr %t166, i64 0
  %t170 = add i64 0, 1
  %t168 = sub nsw i64 %t6, %t170
  %t171 = getelementptr [1024 x i64], ptr %t167, i64 0, i64 %t168
  store i64 %t153, ptr %t171
  br label %guard.end132
  guard.end132:
  %t175 = add i64 0, 1
  %t173 = add nsw i64 %arg8, %t175
  ret i64 %t173
.smt_body_8_4:
  %t179 = add i64 0, 2
  %t180 = icmp sge i64 %t6, %t179
  %t177 = zext i1 %t180 to i8
  %t182 = trunc i8 %t177 to i1
  br i1 %t182, label %guard.then181, label %guard.end181
  guard.then181:
  %t187 = inttoptr i64 %ac0 to ptr
  %t188 = getelementptr i8, ptr %t187, i64 0
  %t191 = add i64 0, 2
  %t189 = sub nsw i64 %t6, %t191
  %t192 = getelementptr [1024 x i64], ptr %t188, i64 0, i64 %t189
  %t193 = load i64, ptr %t192
  %t197 = inttoptr i64 %ac0 to ptr
  %t198 = getelementptr i8, ptr %t197, i64 0
  %t201 = add i64 0, 1
  %t199 = sub nsw i64 %t6, %t201
  %t202 = getelementptr [1024 x i64], ptr %t198, i64 0, i64 %t199
  %t203 = load i64, ptr %t202
  %t183 = add nsw i64 %t193, %t203
  %t206 = inttoptr i64 %ac0 to ptr
  %t207 = getelementptr i8, ptr %t206, i64 0
  %t210 = add i64 0, 2
  %t208 = sub nsw i64 %t6, %t210
  %t211 = getelementptr [1024 x i64], ptr %t207, i64 0, i64 %t208
  store i64 %t183, ptr %t211
  %t214 = add i64 0, 1
  %t212 = sub nsw i64 %t6, %t214
  %t216 = inttoptr i64 %ac0 to ptr
  %t217 = getelementptr i8, ptr %t216, i64 8192
  store i64 %t212, ptr %t217
  br label %guard.end181
  guard.end181:
  %t221 = add i64 0, 1
  %t219 = add nsw i64 %arg8, %t221
  ret i64 %t219
.smt_body_8_5:
  %t225 = add i64 0, 2
  %t226 = icmp sge i64 %t6, %t225
  %t223 = zext i1 %t226 to i8
  %t228 = trunc i8 %t223 to i1
  br i1 %t228, label %guard.then227, label %guard.end227
  guard.then227:
  %t233 = inttoptr i64 %ac0 to ptr
  %t234 = getelementptr i8, ptr %t233, i64 0
  %t237 = add i64 0, 2
  %t235 = sub nsw i64 %t6, %t237
  %t238 = getelementptr [1024 x i64], ptr %t234, i64 0, i64 %t235
  %t239 = load i64, ptr %t238
  %t243 = inttoptr i64 %ac0 to ptr
  %t244 = getelementptr i8, ptr %t243, i64 0
  %t247 = add i64 0, 1
  %t245 = sub nsw i64 %t6, %t247
  %t248 = getelementptr [1024 x i64], ptr %t244, i64 0, i64 %t245
  %t249 = load i64, ptr %t248
  %t229 = sub nsw i64 %t239, %t249
  %t252 = inttoptr i64 %ac0 to ptr
  %t253 = getelementptr i8, ptr %t252, i64 0
  %t256 = add i64 0, 2
  %t254 = sub nsw i64 %t6, %t256
  %t257 = getelementptr [1024 x i64], ptr %t253, i64 0, i64 %t254
  store i64 %t229, ptr %t257
  %t260 = add i64 0, 1
  %t258 = sub nsw i64 %t6, %t260
  %t262 = inttoptr i64 %ac0 to ptr
  %t263 = getelementptr i8, ptr %t262, i64 8192
  store i64 %t258, ptr %t263
  br label %guard.end227
  guard.end227:
  %t267 = add i64 0, 1
  %t265 = add nsw i64 %arg8, %t267
  ret i64 %t265
.smt_body_8_6:
  %t271 = add i64 0, 2
  %t272 = icmp sge i64 %t6, %t271
  %t269 = zext i1 %t272 to i8
  %t274 = trunc i8 %t269 to i1
  br i1 %t274, label %guard.then273, label %guard.end273
  guard.then273:
  %t279 = inttoptr i64 %ac0 to ptr
  %t280 = getelementptr i8, ptr %t279, i64 0
  %t283 = add i64 0, 2
  %t281 = sub nsw i64 %t6, %t283
  %t284 = getelementptr [1024 x i64], ptr %t280, i64 0, i64 %t281
  %t285 = load i64, ptr %t284
  %t289 = inttoptr i64 %ac0 to ptr
  %t290 = getelementptr i8, ptr %t289, i64 0
  %t293 = add i64 0, 1
  %t291 = sub nsw i64 %t6, %t293
  %t294 = getelementptr [1024 x i64], ptr %t290, i64 0, i64 %t291
  %t295 = load i64, ptr %t294
  %t275 = mul nsw i64 %t285, %t295
  %t298 = inttoptr i64 %ac0 to ptr
  %t299 = getelementptr i8, ptr %t298, i64 0
  %t302 = add i64 0, 2
  %t300 = sub nsw i64 %t6, %t302
  %t303 = getelementptr [1024 x i64], ptr %t299, i64 0, i64 %t300
  store i64 %t275, ptr %t303
  %t306 = add i64 0, 1
  %t304 = sub nsw i64 %t6, %t306
  %t308 = inttoptr i64 %ac0 to ptr
  %t309 = getelementptr i8, ptr %t308, i64 8192
  store i64 %t304, ptr %t309
  br label %guard.end273
  guard.end273:
  %t313 = add i64 0, 1
  %t311 = add nsw i64 %arg8, %t313
  ret i64 %t311
.smt_body_8_7:
  %t317 = add i64 0, 2
  %t318 = icmp sge i64 %t6, %t317
  %t315 = zext i1 %t318 to i8
  %t320 = trunc i8 %t315 to i1
  br i1 %t320, label %guard.then319, label %guard.end319
  guard.then319:
  %t325 = inttoptr i64 %ac0 to ptr
  %t326 = getelementptr i8, ptr %t325, i64 0
  %t329 = add i64 0, 2
  %t327 = sub nsw i64 %t6, %t329
  %t330 = getelementptr [1024 x i64], ptr %t326, i64 0, i64 %t327
  %t331 = load i64, ptr %t330
  %t335 = inttoptr i64 %ac0 to ptr
  %t336 = getelementptr i8, ptr %t335, i64 0
  %t339 = add i64 0, 1
  %t337 = sub nsw i64 %t6, %t339
  %t340 = getelementptr [1024 x i64], ptr %t336, i64 0, i64 %t337
  %t341 = load i64, ptr %t340
  %t321 = and i64 %t331, %t341
  %t344 = inttoptr i64 %ac0 to ptr
  %t345 = getelementptr i8, ptr %t344, i64 0
  %t348 = add i64 0, 2
  %t346 = sub nsw i64 %t6, %t348
  %t349 = getelementptr [1024 x i64], ptr %t345, i64 0, i64 %t346
  store i64 %t321, ptr %t349
  %t352 = add i64 0, 1
  %t350 = sub nsw i64 %t6, %t352
  %t354 = inttoptr i64 %ac0 to ptr
  %t355 = getelementptr i8, ptr %t354, i64 8192
  store i64 %t350, ptr %t355
  br label %guard.end319
  guard.end319:
  %t359 = add i64 0, 1
  %t357 = add nsw i64 %arg8, %t359
  ret i64 %t357
.smt_body_8_8:
  %t363 = add i64 0, 2
  %t364 = icmp sge i64 %t6, %t363
  %t361 = zext i1 %t364 to i8
  %t366 = trunc i8 %t361 to i1
  br i1 %t366, label %guard.then365, label %guard.end365
  guard.then365:
  %t371 = inttoptr i64 %ac0 to ptr
  %t372 = getelementptr i8, ptr %t371, i64 0
  %t375 = add i64 0, 2
  %t373 = sub nsw i64 %t6, %t375
  %t376 = getelementptr [1024 x i64], ptr %t372, i64 0, i64 %t373
  %t377 = load i64, ptr %t376
  %t381 = inttoptr i64 %ac0 to ptr
  %t382 = getelementptr i8, ptr %t381, i64 0
  %t385 = add i64 0, 1
  %t383 = sub nsw i64 %t6, %t385
  %t386 = getelementptr [1024 x i64], ptr %t382, i64 0, i64 %t383
  %t387 = load i64, ptr %t386
  %t367 = or i64 %t377, %t387
  %t390 = inttoptr i64 %ac0 to ptr
  %t391 = getelementptr i8, ptr %t390, i64 0
  %t394 = add i64 0, 2
  %t392 = sub nsw i64 %t6, %t394
  %t395 = getelementptr [1024 x i64], ptr %t391, i64 0, i64 %t392
  store i64 %t367, ptr %t395
  %t398 = add i64 0, 1
  %t396 = sub nsw i64 %t6, %t398
  %t400 = inttoptr i64 %ac0 to ptr
  %t401 = getelementptr i8, ptr %t400, i64 8192
  store i64 %t396, ptr %t401
  br label %guard.end365
  guard.end365:
  %t405 = add i64 0, 1
  %t403 = add nsw i64 %arg8, %t405
  ret i64 %t403
.smt_body_8_9:
  %t409 = add i64 0, 2
  %t410 = icmp sge i64 %t6, %t409
  %t407 = zext i1 %t410 to i8
  %t412 = trunc i8 %t407 to i1
  br i1 %t412, label %guard.then411, label %guard.end411
  guard.then411:
  %t417 = inttoptr i64 %ac0 to ptr
  %t418 = getelementptr i8, ptr %t417, i64 0
  %t421 = add i64 0, 2
  %t419 = sub nsw i64 %t6, %t421
  %t422 = getelementptr [1024 x i64], ptr %t418, i64 0, i64 %t419
  %t423 = load i64, ptr %t422
  %t427 = inttoptr i64 %ac0 to ptr
  %t428 = getelementptr i8, ptr %t427, i64 0
  %t431 = add i64 0, 1
  %t429 = sub nsw i64 %t6, %t431
  %t432 = getelementptr [1024 x i64], ptr %t428, i64 0, i64 %t429
  %t433 = load i64, ptr %t432
  %t413 = xor i64 %t423, %t433
  %t436 = inttoptr i64 %ac0 to ptr
  %t437 = getelementptr i8, ptr %t436, i64 0
  %t440 = add i64 0, 2
  %t438 = sub nsw i64 %t6, %t440
  %t441 = getelementptr [1024 x i64], ptr %t437, i64 0, i64 %t438
  store i64 %t413, ptr %t441
  %t444 = add i64 0, 1
  %t442 = sub nsw i64 %t6, %t444
  %t446 = inttoptr i64 %ac0 to ptr
  %t447 = getelementptr i8, ptr %t446, i64 8192
  store i64 %t442, ptr %t447
  br label %guard.end411
  guard.end411:
  %t451 = add i64 0, 1
  %t449 = add nsw i64 %arg8, %t451
  ret i64 %t449
.smt_body_8_10:
  %t455 = add i64 0, 1
  %t456 = icmp sge i64 %t6, %t455
  %t453 = zext i1 %t456 to i8
  %t458 = trunc i8 %t453 to i1
  br i1 %t458, label %guard.then457, label %guard.end457
  guard.then457:
  %t463 = inttoptr i64 %ac0 to ptr
  %t464 = getelementptr i8, ptr %t463, i64 0
  %t467 = add i64 0, 1
  %t465 = sub nsw i64 %t6, %t467
  %t468 = getelementptr [1024 x i64], ptr %t464, i64 0, i64 %t465
  %t469 = load i64, ptr %t468
  %t470 = add i64 0, 0
  %t471 = icmp eq i64 %t469, %t470
  %t459 = zext i1 %t471 to i8
  %t473 = trunc i8 %t459 to i1
  br i1 %t473, label %guard.then472, label %guard.end472
  guard.then472:
  %t474 = add i64 0, 1
  %t477 = inttoptr i64 %ac0 to ptr
  %t478 = getelementptr i8, ptr %t477, i64 0
  %t481 = add i64 0, 1
  %t479 = sub nsw i64 %t6, %t481
  %t482 = getelementptr [1024 x i64], ptr %t478, i64 0, i64 %t479
  store i64 %t474, ptr %t482
  br label %guard.end472
  guard.end472:
  %t488 = inttoptr i64 %ac0 to ptr
  %t489 = getelementptr i8, ptr %t488, i64 0
  %t492 = add i64 0, 1
  %t490 = sub nsw i64 %t6, %t492
  %t493 = getelementptr [1024 x i64], ptr %t489, i64 0, i64 %t490
  %t494 = load i64, ptr %t493
  %t495 = add i64 0, 0
  %t496 = icmp ne i64 %t494, %t495
  %t484 = zext i1 %t496 to i8
  %t498 = trunc i8 %t484 to i1
  br i1 %t498, label %guard.then497, label %guard.end497
  guard.then497:
  %t499 = add i64 0, 0
  %t502 = inttoptr i64 %ac0 to ptr
  %t503 = getelementptr i8, ptr %t502, i64 0
  %t506 = add i64 0, 1
  %t504 = sub nsw i64 %t6, %t506
  %t507 = getelementptr [1024 x i64], ptr %t503, i64 0, i64 %t504
  store i64 %t499, ptr %t507
  br label %guard.end497
  guard.end497:
  br label %guard.end457
  guard.end457:
  %t512 = add i64 0, 1
  %t510 = add nsw i64 %arg8, %t512
  ret i64 %t510
.smt_body_8_11:
  %t516 = add i64 0, 2
  %t517 = icmp sge i64 %t6, %t516
  %t514 = zext i1 %t517 to i8
  %t519 = trunc i8 %t514 to i1
  br i1 %t519, label %guard.then518, label %guard.end518
  guard.then518:
  %t524 = inttoptr i64 %ac0 to ptr
  %t525 = getelementptr i8, ptr %t524, i64 0
  %t528 = add i64 0, 2
  %t526 = sub nsw i64 %t6, %t528
  %t529 = getelementptr [1024 x i64], ptr %t525, i64 0, i64 %t526
  %t530 = load i64, ptr %t529
  %t535 = inttoptr i64 %ac0 to ptr
  %t536 = getelementptr i8, ptr %t535, i64 0
  %t539 = add i64 0, 1
  %t537 = sub nsw i64 %t6, %t539
  %t540 = getelementptr [1024 x i64], ptr %t536, i64 0, i64 %t537
  %t541 = load i64, ptr %t540
  %t542 = add i64 0, 63
  %t531 = and i64 %t541, %t542
  %t520 = shl i64 %t530, %t531
  %t545 = inttoptr i64 %ac0 to ptr
  %t546 = getelementptr i8, ptr %t545, i64 0
  %t549 = add i64 0, 2
  %t547 = sub nsw i64 %t6, %t549
  %t550 = getelementptr [1024 x i64], ptr %t546, i64 0, i64 %t547
  store i64 %t520, ptr %t550
  %t553 = add i64 0, 1
  %t551 = sub nsw i64 %t6, %t553
  %t555 = inttoptr i64 %ac0 to ptr
  %t556 = getelementptr i8, ptr %t555, i64 8192
  store i64 %t551, ptr %t556
  br label %guard.end518
  guard.end518:
  %t560 = add i64 0, 1
  %t558 = add nsw i64 %arg8, %t560
  ret i64 %t558
.smt_body_8_12:
  %t564 = add i64 0, 2
  %t565 = icmp sge i64 %t6, %t564
  %t562 = zext i1 %t565 to i8
  %t567 = trunc i8 %t562 to i1
  br i1 %t567, label %guard.then566, label %guard.end566
  guard.then566:
  %t572 = inttoptr i64 %ac0 to ptr
  %t573 = getelementptr i8, ptr %t572, i64 0
  %t576 = add i64 0, 2
  %t574 = sub nsw i64 %t6, %t576
  %t577 = getelementptr [1024 x i64], ptr %t573, i64 0, i64 %t574
  %t578 = load i64, ptr %t577
  %t583 = inttoptr i64 %ac0 to ptr
  %t584 = getelementptr i8, ptr %t583, i64 0
  %t587 = add i64 0, 1
  %t585 = sub nsw i64 %t6, %t587
  %t588 = getelementptr [1024 x i64], ptr %t584, i64 0, i64 %t585
  %t589 = load i64, ptr %t588
  %t590 = add i64 0, 63
  %t579 = and i64 %t589, %t590
  %t568 = ashr i64 %t578, %t579
  %t593 = inttoptr i64 %ac0 to ptr
  %t594 = getelementptr i8, ptr %t593, i64 0
  %t597 = add i64 0, 2
  %t595 = sub nsw i64 %t6, %t597
  %t598 = getelementptr [1024 x i64], ptr %t594, i64 0, i64 %t595
  store i64 %t568, ptr %t598
  %t601 = add i64 0, 1
  %t599 = sub nsw i64 %t6, %t601
  %t603 = inttoptr i64 %ac0 to ptr
  %t604 = getelementptr i8, ptr %t603, i64 8192
  store i64 %t599, ptr %t604
  br label %guard.end566
  guard.end566:
  %t608 = add i64 0, 1
  %t606 = add nsw i64 %arg8, %t608
  ret i64 %t606
.smt_body_8_13:
  %t612 = add i64 0, 2
  %t613 = icmp sge i64 %t6, %t612
  %t610 = zext i1 %t613 to i8
  %t615 = trunc i8 %t610 to i1
  br i1 %t615, label %guard.then614, label %guard.end614
  guard.then614:
  %t620 = inttoptr i64 %ac0 to ptr
  %t621 = getelementptr i8, ptr %t620, i64 0
  %t624 = add i64 0, 2
  %t622 = sub nsw i64 %t6, %t624
  %t625 = getelementptr [1024 x i64], ptr %t621, i64 0, i64 %t622
  %t626 = load i64, ptr %t625
  %t630 = inttoptr i64 %ac0 to ptr
  %t631 = getelementptr i8, ptr %t630, i64 0
  %t634 = add i64 0, 1
  %t632 = sub nsw i64 %t6, %t634
  %t635 = getelementptr [1024 x i64], ptr %t631, i64 0, i64 %t632
  %t636 = load i64, ptr %t635
  %t637 = icmp eq i64 %t626, %t636
  %t616 = zext i1 %t637 to i8
  %t639 = trunc i8 %t616 to i1
  br i1 %t639, label %guard.then638, label %guard.end638
  guard.then638:
  %t640 = add i64 0, 1
  %t643 = inttoptr i64 %ac0 to ptr
  %t644 = getelementptr i8, ptr %t643, i64 0
  %t647 = add i64 0, 2
  %t645 = sub nsw i64 %t6, %t647
  %t648 = getelementptr [1024 x i64], ptr %t644, i64 0, i64 %t645
  store i64 %t640, ptr %t648
  br label %guard.end638
  guard.end638:
  %t654 = inttoptr i64 %ac0 to ptr
  %t655 = getelementptr i8, ptr %t654, i64 0
  %t658 = add i64 0, 2
  %t656 = sub nsw i64 %t6, %t658
  %t659 = getelementptr [1024 x i64], ptr %t655, i64 0, i64 %t656
  %t660 = load i64, ptr %t659
  %t664 = inttoptr i64 %ac0 to ptr
  %t665 = getelementptr i8, ptr %t664, i64 0
  %t668 = add i64 0, 1
  %t666 = sub nsw i64 %t6, %t668
  %t669 = getelementptr [1024 x i64], ptr %t665, i64 0, i64 %t666
  %t670 = load i64, ptr %t669
  %t671 = icmp ne i64 %t660, %t670
  %t650 = zext i1 %t671 to i8
  %t673 = trunc i8 %t650 to i1
  br i1 %t673, label %guard.then672, label %guard.end672
  guard.then672:
  %t674 = add i64 0, 0
  %t677 = inttoptr i64 %ac0 to ptr
  %t678 = getelementptr i8, ptr %t677, i64 0
  %t681 = add i64 0, 2
  %t679 = sub nsw i64 %t6, %t681
  %t682 = getelementptr [1024 x i64], ptr %t678, i64 0, i64 %t679
  store i64 %t674, ptr %t682
  br label %guard.end672
  guard.end672:
  %t686 = add i64 0, 1
  %t684 = sub nsw i64 %t6, %t686
  %t688 = inttoptr i64 %ac0 to ptr
  %t689 = getelementptr i8, ptr %t688, i64 8192
  store i64 %t684, ptr %t689
  br label %guard.end614
  guard.end614:
  %t693 = add i64 0, 1
  %t691 = add nsw i64 %arg8, %t693
  ret i64 %t691
.smt_body_8_14:
  %t697 = add i64 0, 1
  %t698 = icmp sge i64 %t6, %t697
  %t695 = zext i1 %t698 to i8
  %t700 = trunc i8 %t695 to i1
  br i1 %t700, label %guard.then699, label %guard.end699
  guard.then699:
  %t704 = inttoptr i64 %ac0 to ptr
  %t705 = getelementptr i8, ptr %t704, i64 0
  %t708 = add i64 0, 1
  %t706 = sub nsw i64 %t6, %t708
  %t709 = getelementptr [1024 x i64], ptr %t705, i64 0, i64 %t706
  %t710 = load i64, ptr %t709
  %t713 = inttoptr i64 %t710 to ptr
  %t711 = load i64, ptr %t713
  %t716 = inttoptr i64 %ac0 to ptr
  %t717 = getelementptr i8, ptr %t716, i64 0
  %t720 = add i64 0, 1
  %t718 = sub nsw i64 %t6, %t720
  %t721 = getelementptr [1024 x i64], ptr %t717, i64 0, i64 %t718
  store i64 %t711, ptr %t721
  br label %guard.end699
  guard.end699:
  %t725 = add i64 0, 1
  %t723 = add nsw i64 %arg8, %t725
  ret i64 %t723
.smt_body_8_15:
  %t729 = add i64 0, 2
  %t730 = icmp sge i64 %t6, %t729
  %t727 = zext i1 %t730 to i8
  %t732 = trunc i8 %t727 to i1
  br i1 %t732, label %guard.then731, label %guard.end731
  guard.then731:
  %t736 = inttoptr i64 %ac0 to ptr
  %t737 = getelementptr i8, ptr %t736, i64 0
  %t740 = add i64 0, 2
  %t738 = sub nsw i64 %t6, %t740
  %t741 = getelementptr [1024 x i64], ptr %t737, i64 0, i64 %t738
  %t742 = load i64, ptr %t741
  %t746 = inttoptr i64 %ac0 to ptr
  %t747 = getelementptr i8, ptr %t746, i64 0
  %t750 = add i64 0, 1
  %t748 = sub nsw i64 %t6, %t750
  %t751 = getelementptr [1024 x i64], ptr %t747, i64 0, i64 %t748
  %t752 = load i64, ptr %t751
  %t756 = inttoptr i64 %t742 to ptr
  store i64 %t752, ptr %t756
  %t753 = add i64 0, 0
  %t759 = add i64 0, 2
  %t757 = sub nsw i64 %t6, %t759
  %t761 = inttoptr i64 %ac0 to ptr
  %t762 = getelementptr i8, ptr %t761, i64 8192
  store i64 %t757, ptr %t762
  br label %guard.end731
  guard.end731:
  %t766 = add i64 0, 1
  %t764 = add nsw i64 %arg8, %t766
  ret i64 %t764
.smt_body_8_16:
  %t769 = add i64 0, 1
  %t768 = sub i64 0, %t769
  ret i64 %t768
.smt_body_8_17:
  %t772 = add i64 0, 1
  %t771 = sub i64 0, %t772
  ret i64 %t771
.smt_body_8_18:
  %t778 = add i64 0, 1
  %t776 = add nsw i64 %arg8, %t778
  %t779 = inttoptr i64 %ac3 to ptr
  %t774 = call i64 @read_i8(ptr %state, ptr %t779, i64 %t776)
  %t782 = add i64 0, 1024
  %t783 = icmp slt i64 %t6, %t782
  %t780 = zext i1 %t783 to i8
  %t785 = trunc i8 %t780 to i1
  br i1 %t785, label %guard.then784, label %guard.end784
  guard.then784:
  %t789 = inttoptr i64 %ac0 to ptr
  %t790 = getelementptr i8, ptr %t789, i64 0
  %t792 = getelementptr [1024 x i64], ptr %t790, i64 0, i64 %t6
  store i64 %t774, ptr %t792
  %t795 = add i64 0, 1
  %t793 = add nsw i64 %t6, %t795
  %t797 = inttoptr i64 %ac0 to ptr
  %t798 = getelementptr i8, ptr %t797, i64 8192
  store i64 %t793, ptr %t798
  br label %guard.end784
  guard.end784:
  %t802 = add i64 0, 2
  %t800 = add nsw i64 %arg8, %t802
  ret i64 %t800
.smt_body_8_19:
  %t808 = add i64 0, 1
  %t806 = add nsw i64 %arg8, %t808
  %t809 = inttoptr i64 %ac3 to ptr
  %t804 = call i64 @read_u8(ptr %state, ptr %t809, i64 %t806)
  %t813 = inttoptr i64 %ac1 to ptr
  %t814 = getelementptr i8, ptr %t813, i64 0
  %t815 = add nsw i64 %arg9, %t804
  %t818 = getelementptr [4096 x i64], ptr %t814, i64 0, i64 %t815
  %t819 = load i64, ptr %t818
  %t822 = add i64 0, 1024
  %t823 = icmp slt i64 %t6, %t822
  %t820 = zext i1 %t823 to i8
  %t825 = trunc i8 %t820 to i1
  br i1 %t825, label %guard.then824, label %guard.end824
  guard.then824:
  %t829 = inttoptr i64 %ac0 to ptr
  %t830 = getelementptr i8, ptr %t829, i64 0
  %t832 = getelementptr [1024 x i64], ptr %t830, i64 0, i64 %t6
  store i64 %t819, ptr %t832
  %t835 = add i64 0, 1
  %t833 = add nsw i64 %t6, %t835
  %t837 = inttoptr i64 %ac0 to ptr
  %t838 = getelementptr i8, ptr %t837, i64 8192
  store i64 %t833, ptr %t838
  br label %guard.end824
  guard.end824:
  %t842 = add i64 0, 2
  %t840 = add nsw i64 %arg8, %t842
  ret i64 %t840
.smt_body_8_20:
  %t848 = add i64 0, 1
  %t846 = add nsw i64 %arg8, %t848
  %t849 = inttoptr i64 %ac3 to ptr
  %t844 = call i64 @read_u8(ptr %state, ptr %t849, i64 %t846)
  %t852 = add i64 0, 1
  %t853 = icmp sge i64 %t6, %t852
  %t850 = zext i1 %t853 to i8
  %t855 = trunc i8 %t850 to i1
  br i1 %t855, label %guard.then854, label %guard.end854
  guard.then854:
  %t859 = inttoptr i64 %ac0 to ptr
  %t860 = getelementptr i8, ptr %t859, i64 0
  %t863 = add i64 0, 1
  %t861 = sub nsw i64 %t6, %t863
  %t864 = getelementptr [1024 x i64], ptr %t860, i64 0, i64 %t861
  %t865 = load i64, ptr %t864
  %t868 = inttoptr i64 %ac1 to ptr
  %t869 = getelementptr i8, ptr %t868, i64 0
  %t870 = add nsw i64 %arg9, %t844
  %t873 = getelementptr [4096 x i64], ptr %t869, i64 0, i64 %t870
  store i64 %t865, ptr %t873
  %t876 = add i64 0, 1
  %t874 = sub nsw i64 %t6, %t876
  %t878 = inttoptr i64 %ac0 to ptr
  %t879 = getelementptr i8, ptr %t878, i64 8192
  store i64 %t874, ptr %t879
  br label %guard.end854
  guard.end854:
  %t883 = add i64 0, 2
  %t881 = add nsw i64 %arg8, %t883
  ret i64 %t881
.smt_body_8_21:
  %t889 = add i64 0, 1
  %t887 = add nsw i64 %arg8, %t889
  %t890 = inttoptr i64 %ac3 to ptr
  %t885 = call i64 @read_u8(ptr %state, ptr %t890, i64 %t887)
    %t896 = inttoptr i64 %ac2 to ptr
    %t897 = getelementptr i8, ptr %t896, i64 6144
    %t898 = load i64, ptr %t897
  %t899 = add i64 0, 256
  %t900 = icmp slt i64 %t898, %t899
  %t891 = zext i1 %t900 to i8
  %t902 = trunc i8 %t891 to i1
  br i1 %t902, label %guard.then901, label %guard.end901
  guard.then901:
    %t907 = inttoptr i64 %ac1 to ptr
    %t908 = getelementptr i8, ptr %t907, i64 32768
    %t909 = load i64, ptr %t908
  %t913 = inttoptr i64 %ac2 to ptr
  %t914 = getelementptr i8, ptr %t913, i64 0
    %t919 = inttoptr i64 %ac2 to ptr
    %t920 = getelementptr i8, ptr %t919, i64 6144
    %t921 = load i64, ptr %t920
  %t922 = mul i64 %t921, 24
  %t923 = getelementptr i8, ptr %t914, i64 %t922
  %t924 = ptrtoint ptr %t923 to i64
  %t925 = inttoptr i64 %t924 to ptr
  %t926 = getelementptr i8, ptr %t925, i64 0
  store i64 %t909, ptr %t926
  %t931 = inttoptr i64 %ac2 to ptr
  %t932 = getelementptr i8, ptr %t931, i64 0
    %t937 = inttoptr i64 %ac2 to ptr
    %t938 = getelementptr i8, ptr %t937, i64 6144
    %t939 = load i64, ptr %t938
  %t940 = mul i64 %t939, 24
  %t941 = getelementptr i8, ptr %t932, i64 %t940
  %t942 = ptrtoint ptr %t941 to i64
  %t943 = inttoptr i64 %t942 to ptr
  %t944 = getelementptr i8, ptr %t943, i64 8
  store i64 %t885, ptr %t944
  %t947 = add i64 0, 2
  %t945 = add nsw i64 %arg8, %t947
  %t951 = inttoptr i64 %ac2 to ptr
  %t952 = getelementptr i8, ptr %t951, i64 0
    %t957 = inttoptr i64 %ac2 to ptr
    %t958 = getelementptr i8, ptr %t957, i64 6144
    %t959 = load i64, ptr %t958
  %t960 = mul i64 %t959, 24
  %t961 = getelementptr i8, ptr %t952, i64 %t960
  %t962 = ptrtoint ptr %t961 to i64
  %t963 = inttoptr i64 %t962 to ptr
  %t964 = getelementptr i8, ptr %t963, i64 16
  store i64 %t945, ptr %t964
    %t970 = inttoptr i64 %ac2 to ptr
    %t971 = getelementptr i8, ptr %t970, i64 6144
    %t972 = load i64, ptr %t971
  %t973 = add i64 0, 1
  %t965 = add nsw i64 %t972, %t973
  %t975 = inttoptr i64 %ac2 to ptr
  %t976 = getelementptr i8, ptr %t975, i64 6144
  store i64 %t965, ptr %t976
  br label %guard.end901
  guard.end901:
  %t980 = add i64 0, 2
  %t978 = add nsw i64 %arg8, %t980
  ret i64 %t978
.smt_body_8_22:
    %t987 = inttoptr i64 %ac2 to ptr
    %t988 = getelementptr i8, ptr %t987, i64 6144
    %t989 = load i64, ptr %t988
  %t990 = add i64 0, 0
  %t991 = icmp sgt i64 %t989, %t990
  %t982 = zext i1 %t991 to i8
  %t993 = trunc i8 %t982 to i1
  br i1 %t993, label %guard.then992, label %guard.end992
  guard.then992:
    %t999 = inttoptr i64 %ac2 to ptr
    %t1000 = getelementptr i8, ptr %t999, i64 6144
    %t1001 = load i64, ptr %t1000
  %t1002 = add i64 0, 1
  %t994 = sub nsw i64 %t1001, %t1002
  %t1004 = inttoptr i64 %ac2 to ptr
  %t1005 = getelementptr i8, ptr %t1004, i64 6144
  store i64 %t994, ptr %t1005
  br label %guard.end992
  guard.end992:
  %t1009 = add i64 0, 1
  %t1007 = add nsw i64 %arg8, %t1009
  ret i64 %t1007
.smt_body_8_23:
  %t1015 = add i64 0, 1
  %t1013 = add nsw i64 %arg8, %t1015
  %t1016 = inttoptr i64 %ac3 to ptr
  %t1011 = call i64 @read_i16(ptr %state, ptr %t1016, i64 %t1013)
  %t1019 = add i64 0, 1024
  %t1020 = icmp slt i64 %t6, %t1019
  %t1017 = zext i1 %t1020 to i8
  %t1022 = trunc i8 %t1017 to i1
  br i1 %t1022, label %guard.then1021, label %guard.end1021
  guard.then1021:
  %t1026 = inttoptr i64 %ac0 to ptr
  %t1027 = getelementptr i8, ptr %t1026, i64 0
  %t1029 = getelementptr [1024 x i64], ptr %t1027, i64 0, i64 %t6
  store i64 %t1011, ptr %t1029
  %t1032 = add i64 0, 1
  %t1030 = add nsw i64 %t6, %t1032
  %t1034 = inttoptr i64 %ac0 to ptr
  %t1035 = getelementptr i8, ptr %t1034, i64 8192
  store i64 %t1030, ptr %t1035
  br label %guard.end1021
  guard.end1021:
  %t1039 = add i64 0, 3
  %t1037 = add nsw i64 %arg8, %t1039
  ret i64 %t1037
.smt_body_8_24:
  %t1045 = add i64 0, 1
  %t1043 = add nsw i64 %arg8, %t1045
  %t1046 = inttoptr i64 %ac3 to ptr
  %t1041 = call i64 @read_i16(ptr %state, ptr %t1046, i64 %t1043)
  %t1050 = add i64 0, 3
  %t1048 = add nsw i64 %arg8, %t1050
  %t1047 = add nsw i64 %t1048, %t1041
  ret i64 %t1047
.smt_body_8_25:
  %t1057 = add i64 0, 1
  %t1055 = add nsw i64 %arg8, %t1057
  %t1058 = inttoptr i64 %ac3 to ptr
  %t1053 = call i64 @read_i16(ptr %state, ptr %t1058, i64 %t1055)
  %t1061 = add i64 0, 1
  %t1062 = icmp sge i64 %t6, %t1061
  %t1059 = zext i1 %t1062 to i8
  %t1064 = trunc i8 %t1059 to i1
  br i1 %t1064, label %guard.then1063, label %guard.end1063
  guard.then1063:
  %t1068 = inttoptr i64 %ac0 to ptr
  %t1069 = getelementptr i8, ptr %t1068, i64 0
  %t1072 = add i64 0, 1
  %t1070 = sub nsw i64 %t6, %t1072
  %t1073 = getelementptr [1024 x i64], ptr %t1069, i64 0, i64 %t1070
  %t1074 = load i64, ptr %t1073
  %t1077 = add i64 0, 1
  %t1075 = sub nsw i64 %t6, %t1077
  %t1079 = inttoptr i64 %ac0 to ptr
  %t1080 = getelementptr i8, ptr %t1079, i64 8192
  store i64 %t1075, ptr %t1080
  %t1083 = add i64 0, 0
  %t1084 = icmp eq i64 %t1074, %t1083
  %t1081 = zext i1 %t1084 to i8
  %t1086 = trunc i8 %t1081 to i1
  br i1 %t1086, label %guard.then1085, label %guard.end1085
  guard.then1085:
  %t1090 = add i64 0, 3
  %t1088 = add nsw i64 %arg8, %t1090
  %t1087 = add nsw i64 %t1088, %t1053
  ret i64 %t1087
  guard.end1085:
  %t1096 = add i64 0, 3
  %t1094 = add nsw i64 %arg8, %t1096
  ret i64 %t1094
  guard.end1063:
  %t1101 = add i64 0, 3
  %t1099 = add nsw i64 %arg8, %t1101
  ret i64 %t1099
.smt_body_8_26:
  %t1107 = add i64 0, 1
  %t1105 = add nsw i64 %arg8, %t1107
  %t1108 = inttoptr i64 %ac3 to ptr
  %t1103 = call i64 @read_i16(ptr %state, ptr %t1108, i64 %t1105)
  %t1111 = add i64 0, 1
  %t1112 = icmp sge i64 %t6, %t1111
  %t1109 = zext i1 %t1112 to i8
  %t1114 = trunc i8 %t1109 to i1
  br i1 %t1114, label %guard.then1113, label %guard.end1113
  guard.then1113:
  %t1118 = inttoptr i64 %ac0 to ptr
  %t1119 = getelementptr i8, ptr %t1118, i64 0
  %t1122 = add i64 0, 1
  %t1120 = sub nsw i64 %t6, %t1122
  %t1123 = getelementptr [1024 x i64], ptr %t1119, i64 0, i64 %t1120
  %t1124 = load i64, ptr %t1123
  %t1127 = add i64 0, 1
  %t1125 = sub nsw i64 %t6, %t1127
  %t1129 = inttoptr i64 %ac0 to ptr
  %t1130 = getelementptr i8, ptr %t1129, i64 8192
  store i64 %t1125, ptr %t1130
  %t1133 = add i64 0, 0
  %t1134 = icmp ne i64 %t1124, %t1133
  %t1131 = zext i1 %t1134 to i8
  %t1136 = trunc i8 %t1131 to i1
  br i1 %t1136, label %guard.then1135, label %guard.end1135
  guard.then1135:
  %t1140 = add i64 0, 3
  %t1138 = add nsw i64 %arg8, %t1140
  %t1137 = add nsw i64 %t1138, %t1103
  ret i64 %t1137
  guard.end1135:
  %t1146 = add i64 0, 3
  %t1144 = add nsw i64 %arg8, %t1146
  ret i64 %t1144
  guard.end1113:
  %t1151 = add i64 0, 3
  %t1149 = add nsw i64 %arg8, %t1151
  ret i64 %t1149
.smt_body_8_27:
  %t1157 = add i64 0, 1
  %t1155 = add nsw i64 %arg8, %t1157
  %t1158 = inttoptr i64 %ac3 to ptr
  %t1153 = call i64 @read_u16(ptr %state, ptr %t1158, i64 %t1155)
  %t1162 = icmp slt i64 %t1153, %arg6
  %t1159 = zext i1 %t1162 to i8
  %t1164 = trunc i8 %t1159 to i1
  br i1 %t1164, label %guard.then1163, label %guard.end1163
  guard.then1163:
  %t1169 = inttoptr i64 %ac4 to ptr
  %t1165 = call i64 @fn_bc_offset(ptr %state, ptr %t1169, i64 %arg5, i64 %t1153)
  %t1174 = inttoptr i64 %ac4 to ptr
  %t1170 = call i64 @fn_local_count(ptr %state, ptr %t1174, i64 %arg5, i64 %t1153)
  %t1179 = inttoptr i64 %ac4 to ptr
  %t1175 = call i64 @fn_arg_count(ptr %state, ptr %t1179, i64 %arg5, i64 %t1153)
    %t1185 = inttoptr i64 %ac2 to ptr
    %t1186 = getelementptr i8, ptr %t1185, i64 6144
    %t1187 = load i64, ptr %t1186
  %t1188 = add i64 0, 256
  %t1189 = icmp slt i64 %t1187, %t1188
  %t1180 = zext i1 %t1189 to i8
  %t1191 = trunc i8 %t1180 to i1
  br i1 %t1191, label %guard.then1190, label %guard.end1190
  guard.then1190:
  %t1194 = add i64 0, 3
  %t1192 = add nsw i64 %arg8, %t1194
  %t1198 = inttoptr i64 %ac2 to ptr
  %t1199 = getelementptr i8, ptr %t1198, i64 0
    %t1204 = inttoptr i64 %ac2 to ptr
    %t1205 = getelementptr i8, ptr %t1204, i64 6144
    %t1206 = load i64, ptr %t1205
  %t1207 = mul i64 %t1206, 24
  %t1208 = getelementptr i8, ptr %t1199, i64 %t1207
  %t1209 = ptrtoint ptr %t1208 to i64
  %t1210 = inttoptr i64 %t1209 to ptr
  %t1211 = getelementptr i8, ptr %t1210, i64 16
  store i64 %t1192, ptr %t1211
    %t1216 = inttoptr i64 %ac1 to ptr
    %t1217 = getelementptr i8, ptr %t1216, i64 32768
    %t1218 = load i64, ptr %t1217
  %t1222 = inttoptr i64 %ac2 to ptr
  %t1223 = getelementptr i8, ptr %t1222, i64 0
    %t1228 = inttoptr i64 %ac2 to ptr
    %t1229 = getelementptr i8, ptr %t1228, i64 6144
    %t1230 = load i64, ptr %t1229
  %t1231 = mul i64 %t1230, 24
  %t1232 = getelementptr i8, ptr %t1223, i64 %t1231
  %t1233 = ptrtoint ptr %t1232 to i64
  %t1234 = inttoptr i64 %t1233 to ptr
  %t1235 = getelementptr i8, ptr %t1234, i64 0
  store i64 %t1218, ptr %t1235
  %t1240 = inttoptr i64 %ac2 to ptr
  %t1241 = getelementptr i8, ptr %t1240, i64 0
    %t1246 = inttoptr i64 %ac2 to ptr
    %t1247 = getelementptr i8, ptr %t1246, i64 6144
    %t1248 = load i64, ptr %t1247
  %t1249 = mul i64 %t1248, 24
  %t1250 = getelementptr i8, ptr %t1241, i64 %t1249
  %t1251 = ptrtoint ptr %t1250 to i64
  %t1252 = inttoptr i64 %t1251 to ptr
  %t1253 = getelementptr i8, ptr %t1252, i64 8
  store i64 %t1170, ptr %t1253
    %t1259 = inttoptr i64 %ac2 to ptr
    %t1260 = getelementptr i8, ptr %t1259, i64 6144
    %t1261 = load i64, ptr %t1260
  %t1262 = add i64 0, 1
  %t1254 = add nsw i64 %t1261, %t1262
  %t1264 = inttoptr i64 %ac2 to ptr
  %t1265 = getelementptr i8, ptr %t1264, i64 6144
  store i64 %t1254, ptr %t1265
  %t1268 = add i64 0, 1
  %t1269 = icmp sge i64 %t1175, %t1268
  %t1266 = zext i1 %t1269 to i8
  %t1271 = trunc i8 %t1266 to i1
  br i1 %t1271, label %guard.then1270, label %guard.end1270
  guard.then1270:
  %t1275 = inttoptr i64 %ac0 to ptr
  %t1276 = getelementptr i8, ptr %t1275, i64 0
  %t1279 = add i64 0, 1
  %t1277 = sub nsw i64 %t6, %t1279
  %t1280 = getelementptr [1024 x i64], ptr %t1276, i64 0, i64 %t1277
  %t1281 = load i64, ptr %t1280
  %t1284 = inttoptr i64 %ac1 to ptr
  %t1285 = getelementptr i8, ptr %t1284, i64 0
    %t1291 = inttoptr i64 %ac1 to ptr
    %t1292 = getelementptr i8, ptr %t1291, i64 32768
    %t1293 = load i64, ptr %t1292
  %t1294 = add i64 0, 0
  %t1286 = add nsw i64 %t1293, %t1294
  %t1295 = getelementptr [4096 x i64], ptr %t1285, i64 0, i64 %t1286
  store i64 %t1281, ptr %t1295
  br label %guard.end1270
  guard.end1270:
  %t1299 = add i64 0, 2
  %t1300 = icmp sge i64 %t1175, %t1299
  %t1297 = zext i1 %t1300 to i8
  %t1302 = trunc i8 %t1297 to i1
  br i1 %t1302, label %guard.then1301, label %guard.end1301
  guard.then1301:
  %t1306 = inttoptr i64 %ac0 to ptr
  %t1307 = getelementptr i8, ptr %t1306, i64 0
  %t1310 = add i64 0, 2
  %t1308 = sub nsw i64 %t6, %t1310
  %t1311 = getelementptr [1024 x i64], ptr %t1307, i64 0, i64 %t1308
  %t1312 = load i64, ptr %t1311
  %t1315 = inttoptr i64 %ac1 to ptr
  %t1316 = getelementptr i8, ptr %t1315, i64 0
    %t1322 = inttoptr i64 %ac1 to ptr
    %t1323 = getelementptr i8, ptr %t1322, i64 32768
    %t1324 = load i64, ptr %t1323
  %t1325 = add i64 0, 1
  %t1317 = add nsw i64 %t1324, %t1325
  %t1326 = getelementptr [4096 x i64], ptr %t1316, i64 0, i64 %t1317
  store i64 %t1312, ptr %t1326
  br label %guard.end1301
  guard.end1301:
  %t1330 = add i64 0, 3
  %t1331 = icmp sge i64 %t1175, %t1330
  %t1328 = zext i1 %t1331 to i8
  %t1333 = trunc i8 %t1328 to i1
  br i1 %t1333, label %guard.then1332, label %guard.end1332
  guard.then1332:
  %t1337 = inttoptr i64 %ac0 to ptr
  %t1338 = getelementptr i8, ptr %t1337, i64 0
  %t1341 = add i64 0, 3
  %t1339 = sub nsw i64 %t6, %t1341
  %t1342 = getelementptr [1024 x i64], ptr %t1338, i64 0, i64 %t1339
  %t1343 = load i64, ptr %t1342
  %t1346 = inttoptr i64 %ac1 to ptr
  %t1347 = getelementptr i8, ptr %t1346, i64 0
    %t1353 = inttoptr i64 %ac1 to ptr
    %t1354 = getelementptr i8, ptr %t1353, i64 32768
    %t1355 = load i64, ptr %t1354
  %t1356 = add i64 0, 2
  %t1348 = add nsw i64 %t1355, %t1356
  %t1357 = getelementptr [4096 x i64], ptr %t1347, i64 0, i64 %t1348
  store i64 %t1343, ptr %t1357
  br label %guard.end1332
  guard.end1332:
  %t1361 = add i64 0, 4
  %t1362 = icmp sge i64 %t1175, %t1361
  %t1359 = zext i1 %t1362 to i8
  %t1364 = trunc i8 %t1359 to i1
  br i1 %t1364, label %guard.then1363, label %guard.end1363
  guard.then1363:
  %t1368 = inttoptr i64 %ac0 to ptr
  %t1369 = getelementptr i8, ptr %t1368, i64 0
  %t1372 = add i64 0, 4
  %t1370 = sub nsw i64 %t6, %t1372
  %t1373 = getelementptr [1024 x i64], ptr %t1369, i64 0, i64 %t1370
  %t1374 = load i64, ptr %t1373
  %t1377 = inttoptr i64 %ac1 to ptr
  %t1378 = getelementptr i8, ptr %t1377, i64 0
    %t1384 = inttoptr i64 %ac1 to ptr
    %t1385 = getelementptr i8, ptr %t1384, i64 32768
    %t1386 = load i64, ptr %t1385
  %t1387 = add i64 0, 3
  %t1379 = add nsw i64 %t1386, %t1387
  %t1388 = getelementptr [4096 x i64], ptr %t1378, i64 0, i64 %t1379
  store i64 %t1374, ptr %t1388
  br label %guard.end1363
  guard.end1363:
    %t1395 = inttoptr i64 %ac1 to ptr
    %t1396 = getelementptr i8, ptr %t1395, i64 32768
    %t1397 = load i64, ptr %t1396
  %t1390 = add nsw i64 %t1397, %t1170
  %t1400 = inttoptr i64 %ac1 to ptr
  %t1401 = getelementptr i8, ptr %t1400, i64 32768
  store i64 %t1390, ptr %t1401
  %t1402 = sub nsw i64 %t6, %t1175
  %t1406 = inttoptr i64 %ac0 to ptr
  %t1407 = getelementptr i8, ptr %t1406, i64 8192
  store i64 %t1402, ptr %t1407
  ret i64 %t1165
  guard.end1190:
  %t1413 = add i64 0, 3
  %t1411 = add nsw i64 %arg8, %t1413
  ret i64 %t1411
  guard.end1163:
  %t1418 = add i64 0, 3
  %t1416 = add nsw i64 %arg8, %t1418
  ret i64 %t1416
.smt_body_8_28:
  %t1424 = add i64 0, 1
  %t1422 = add nsw i64 %arg8, %t1424
  %t1425 = inttoptr i64 %ac3 to ptr
  %t1420 = call i64 @read_i32(ptr %state, ptr %t1425, i64 %t1422)
  %t1428 = add i64 0, 1024
  %t1429 = icmp slt i64 %t6, %t1428
  %t1426 = zext i1 %t1429 to i8
  %t1431 = trunc i8 %t1426 to i1
  br i1 %t1431, label %guard.then1430, label %guard.end1430
  guard.then1430:
  %t1435 = inttoptr i64 %ac0 to ptr
  %t1436 = getelementptr i8, ptr %t1435, i64 0
  %t1438 = getelementptr [1024 x i64], ptr %t1436, i64 0, i64 %t6
  store i64 %t1420, ptr %t1438
  %t1441 = add i64 0, 1
  %t1439 = add nsw i64 %t6, %t1441
  %t1443 = inttoptr i64 %ac0 to ptr
  %t1444 = getelementptr i8, ptr %t1443, i64 8192
  store i64 %t1439, ptr %t1444
  br label %guard.end1430
  guard.end1430:
  %t1448 = add i64 0, 5
  %t1446 = add nsw i64 %arg8, %t1448
  ret i64 %t1446
.smt_body_8_29:
  %t1454 = add i64 0, 1
  %t1452 = add nsw i64 %arg8, %t1454
  %t1455 = inttoptr i64 %ac3 to ptr
  %t1450 = call i64 @read_u32(ptr %state, ptr %t1455, i64 %t1452)
   %t1456 = call i64 @briev_host_arity_of(i64 %t1450)
  %t1460 = add i64 0, 1
  %t1461 = icmp eq i64 %t1456, %t1460
  %t1458 = zext i1 %t1461 to i8
  %t1463 = trunc i8 %t1458 to i1
  br i1 %t1463, label %guard.then1462, label %guard.end1462
  guard.then1462:
  %t1467 = inttoptr i64 %ac0 to ptr
  %t1468 = getelementptr i8, ptr %t1467, i64 0
    %t1474 = inttoptr i64 %ac0 to ptr
    %t1475 = getelementptr i8, ptr %t1474, i64 8192
    %t1476 = load i64, ptr %t1475
  %t1477 = add i64 0, 1
  %t1469 = sub nsw i64 %t1476, %t1477
  %t1478 = getelementptr [1024 x i64], ptr %t1468, i64 0, i64 %t1469
  %t1479 = load i64, ptr %t1478
    %t1485 = inttoptr i64 %ac0 to ptr
    %t1486 = getelementptr i8, ptr %t1485, i64 8192
    %t1487 = load i64, ptr %t1486
  %t1488 = add i64 0, 1
  %t1480 = sub nsw i64 %t1487, %t1488
  %t1490 = inttoptr i64 %ac0 to ptr
  %t1491 = getelementptr i8, ptr %t1490, i64 8192
  store i64 %t1480, ptr %t1491
  %t1492 = call i64 @host_dispatch1(ptr %state, i64 %t1450, i64 %t1479)
  br label %guard.end1462
  guard.end1462:
  %t1498 = add i64 0, 5
  %t1496 = add nsw i64 %arg8, %t1498
  ret i64 %t1496
.smt_body_8_30:
  %t1504 = add i64 0, 1
  %t1502 = add nsw i64 %arg8, %t1504
  %t1505 = inttoptr i64 %ac3 to ptr
  %t1500 = call i64 @read_i64(ptr %state, ptr %t1505, i64 %t1502)
  %t1508 = add i64 0, 1024
  %t1509 = icmp slt i64 %t6, %t1508
  %t1506 = zext i1 %t1509 to i8
  %t1511 = trunc i8 %t1506 to i1
  br i1 %t1511, label %guard.then1510, label %guard.end1510
  guard.then1510:
  %t1515 = inttoptr i64 %ac0 to ptr
  %t1516 = getelementptr i8, ptr %t1515, i64 0
  %t1518 = getelementptr [1024 x i64], ptr %t1516, i64 0, i64 %t6
  store i64 %t1500, ptr %t1518
  %t1521 = add i64 0, 1
  %t1519 = add nsw i64 %t6, %t1521
  %t1523 = inttoptr i64 %ac0 to ptr
  %t1524 = getelementptr i8, ptr %t1523, i64 8192
  store i64 %t1519, ptr %t1524
  br label %guard.end1510
  guard.end1510:
  %t1528 = add i64 0, 9
  %t1526 = add nsw i64 %arg8, %t1528
  ret i64 %t1526
.smt_body_8_31:
  %t1532 = add i64 0, 1
  %t1533 = icmp sge i64 %t6, %t1532
  %t1530 = zext i1 %t1533 to i8
  %t1535 = trunc i8 %t1530 to i1
  br i1 %t1535, label %guard.then1534, label %guard.end1534
  guard.then1534:
  %t1540 = inttoptr i64 %ac0 to ptr
  %t1541 = getelementptr i8, ptr %t1540, i64 0
  %t1544 = add i64 0, 1
  %t1542 = sub nsw i64 %t6, %t1544
  %t1545 = getelementptr [1024 x i64], ptr %t1541, i64 0, i64 %t1542
  %t1546 = load i64, ptr %t1545
  %t1536 = xor i64 %t1546, -1
  %t1549 = inttoptr i64 %ac0 to ptr
  %t1550 = getelementptr i8, ptr %t1549, i64 0
  %t1553 = add i64 0, 1
  %t1551 = sub nsw i64 %t6, %t1553
  %t1554 = getelementptr [1024 x i64], ptr %t1550, i64 0, i64 %t1551
  store i64 %t1536, ptr %t1554
  br label %guard.end1534
  guard.end1534:
  %t1558 = add i64 0, 1
  %t1556 = add nsw i64 %arg8, %t1558
  ret i64 %t1556
.smt_body_8_32:
  %t1561 = add i64 0, 1
  %t1560 = sub i64 0, %t1561
  ret i64 %t1560
.smt_end_8:
  unreachable
}

define i64 @host_arity_scan(ptr noundef noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
    %t6 = inttoptr i64 %ac0 to ptr
    %t7 = getelementptr i8, ptr %t6, i64 1024
    %t8 = load i64, ptr %t7
  %t9 = icmp sge i64 %arg2, %t8
  %t0 = zext i1 %t9 to i8
  %t11 = trunc i8 %t0 to i1
  br i1 %t11, label %guard.then10, label %guard.end10
  guard.then10:
  %t13 = add i64 0, 1
  %t12 = sub i64 0, %t13
  ret i64 %t12
  guard.end10:
  %t20 = inttoptr i64 %ac0 to ptr
  %t21 = getelementptr i8, ptr %t20, i64 0
  %t23 = getelementptr [64 x i64], ptr %t21, i64 0, i64 %arg2
  %t24 = load i64, ptr %t23
  %t26 = icmp eq i64 %t24, %arg1
  %t16 = zext i1 %t26 to i8
  %t28 = trunc i8 %t16 to i1
  br i1 %t28, label %guard.then27, label %guard.end27
  guard.then27:
  %t32 = inttoptr i64 %ac0 to ptr
  %t33 = getelementptr i8, ptr %t32, i64 512
  %t35 = getelementptr [64 x i64], ptr %t33, i64 0, i64 %arg2
  %t36 = load i64, ptr %t35
  ret i64 %t36
  guard.end27:
  %t44 = add i64 0, 1
  %t42 = add nsw i64 %arg2, %t44
  %t45 = inttoptr i64 %ac0 to ptr
  %t39 = call i64 @host_arity_scan(ptr %state, ptr %t45, i64 %arg1, i64 %t42)
  ret i64 %t39
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

define i64 @step(ptr noundef noalias nocapture align 8 %state, ptr %arg0, ptr %arg1, ptr %arg2, ptr %arg3, i64 %arg4, ptr %arg5, i64 %arg6, i64 %arg7, i64 %arg8) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac1 = ptrtoint ptr %arg1 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %ac3 = ptrtoint ptr %arg3 to i64
  %ac5 = ptrtoint ptr %arg5 to i64
  %t3 = add i64 0, 0
  %t4 = icmp slt i64 %arg8, %t3
  %t1 = zext i1 %t4 to i8
  %t8 = icmp sge i64 %arg8, %arg4
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
  %t15 = call i64 @read_u8(ptr %state, ptr %t18, i64 %arg8)
  %t29 = add i64 0, 0
  %t30 = inttoptr i64 %ac0 to ptr
  %t31 = inttoptr i64 %ac1 to ptr
  %t32 = inttoptr i64 %ac2 to ptr
  %t33 = inttoptr i64 %ac3 to ptr
  %t34 = inttoptr i64 %ac5 to ptr
  %t19 = call i64 @exec_op(ptr %state, ptr %t30, ptr %t31, ptr %t32, ptr %t33, ptr %t34, i64 %arg6, i64 %arg7, i64 %t15, i64 %arg8, i64 %t29)
  ret i64 %t19
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
  %p0_l36 = load i64, ptr %p0_s, align 8
  %p1_l37 = load i64, ptr %p1_s, align 8
  %p2_l38 = load i64, ptr %p2_s, align 8
  %p3_l39 = load i64, ptr %p3_s, align 8
  %p4_l40 = load i64, ptr %p4_s, align 8
  %p5_l41 = load i64, ptr %p5_s, align 8
  %p6_l42 = load i64, ptr %p6_s, align 8
  %p7_l43 = load i64, ptr %p7_s, align 8
  %p8_l44 = load i64, ptr %p8_s, align 8
  %p9_l45 = load i64, ptr %p9_s, align 8
  %p10_l46 = load i64, ptr %p10_s, align 8
  %p11_l47 = load i64, ptr %p11_s, align 8
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
  %t42 = load i64, ptr %p9_s, align 8
  %t44 = load i64, ptr %p10_s, align 8
  %t45 = inttoptr i64 %t27 to ptr
  %t46 = inttoptr i64 %t29 to ptr
  %t47 = inttoptr i64 %t31 to ptr
  %t48 = inttoptr i64 %t33 to ptr
  %t49 = inttoptr i64 %t35 to ptr
  %t25 = call i64 @exec_op(ptr %state, ptr %t45, ptr %t46, ptr %t47, ptr %t48, ptr %t49, i64 %t37, i64 %t39, i64 %t19, i64 %t42, i64 %t44)
  %t52 = add i64 0, 0
  %t53 = icmp slt i64 %t25, %t52
  %t50 = zext i1 %t53 to i8
  %t55 = trunc i8 %t50 to i1
  br i1 %t55, label %guard.then54, label %guard.end54
  guard.then54:
  %t56 = add i64 0, 0
  store i64 %t56, ptr %p11_s
  br label %guard.end54
  guard.end54:
  %t61 = add i64 0, 0
  %t62 = icmp sge i64 %t25, %t61
  %t59 = zext i1 %t62 to i8
  %t66 = load i64, ptr %p4_s, align 8
  %t67 = icmp slt i64 %t25, %t66
  %t63 = zext i1 %t67 to i8
  %t58 = and i8 %t59, %t63
  %t69 = trunc i8 %t58 to i1
  br i1 %t69, label %guard.then68, label %guard.end68
  guard.then68:
  store i64 %t25, ptr %p9_s
  br label %guard.end68
  guard.end68:
  %t72 = add i64 0, 0
  store i64 %t72, ptr %result
  br label %post
post:
  br label %loop
done:
  %ret74 = load i64, ptr %result, align 8
  ret i64 %ret74
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
