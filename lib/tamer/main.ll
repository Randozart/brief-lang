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
  %t23 = add i64 0, 9
  %t24 = icmp eq i64 %arg7, %t23
  br i1 %t24, label %.smt_body_8_7, label %.smt_next_8_7
.smt_next_8_7:
  %t25 = add i64 0, 10
  %t26 = icmp eq i64 %arg7, %t25
  br i1 %t26, label %.smt_body_8_8, label %.smt_next_8_8
.smt_next_8_8:
  %t27 = add i64 0, 11
  %t28 = icmp eq i64 %arg7, %t27
  br i1 %t28, label %.smt_body_8_9, label %.smt_next_8_9
.smt_next_8_9:
  %t29 = add i64 0, 12
  %t30 = icmp eq i64 %arg7, %t29
  br i1 %t30, label %.smt_body_8_10, label %.smt_next_8_10
.smt_next_8_10:
  %t31 = add i64 0, 13
  %t32 = icmp eq i64 %arg7, %t31
  br i1 %t32, label %.smt_body_8_11, label %.smt_next_8_11
.smt_next_8_11:
  %t33 = add i64 0, 14
  %t34 = icmp eq i64 %arg7, %t33
  br i1 %t34, label %.smt_body_8_12, label %.smt_next_8_12
.smt_next_8_12:
  %t35 = add i64 0, 15
  %t36 = icmp eq i64 %arg7, %t35
  br i1 %t36, label %.smt_body_8_13, label %.smt_next_8_13
.smt_next_8_13:
  %t37 = add i64 0, 16
  %t38 = icmp eq i64 %arg7, %t37
  br i1 %t38, label %.smt_body_8_14, label %.smt_next_8_14
.smt_next_8_14:
  %t39 = add i64 0, 17
  %t40 = icmp eq i64 %arg7, %t39
  br i1 %t40, label %.smt_body_8_15, label %.smt_next_8_15
.smt_next_8_15:
  %t41 = add i64 0, 23
  %t42 = icmp eq i64 %arg7, %t41
  br i1 %t42, label %.smt_body_8_16, label %.smt_next_8_16
.smt_next_8_16:
  %t43 = add i64 0, 24
  %t44 = icmp eq i64 %arg7, %t43
  br i1 %t44, label %.smt_body_8_17, label %.smt_next_8_17
.smt_next_8_17:
  %t45 = add i64 0, 25
  %t46 = icmp eq i64 %arg7, %t45
  br i1 %t46, label %.smt_body_8_18, label %.smt_next_8_18
.smt_next_8_18:
  %t47 = add i64 0, 27
  %t48 = icmp eq i64 %arg7, %t47
  br i1 %t48, label %.smt_body_8_19, label %.smt_next_8_19
.smt_next_8_19:
  %t49 = add i64 0, 48
  %t50 = icmp eq i64 %arg7, %t49
  br i1 %t50, label %.smt_body_8_20, label %.smt_next_8_20
.smt_next_8_20:
  %t51 = add i64 0, 49
  %t52 = icmp eq i64 %arg7, %t51
  br i1 %t52, label %.smt_body_8_21, label %.smt_next_8_21
.smt_next_8_21:
  %t53 = add i64 0, 50
  %t54 = icmp eq i64 %arg7, %t53
  br i1 %t54, label %.smt_body_8_22, label %.smt_next_8_22
.smt_next_8_22:
  %t55 = add i64 0, 51
  %t56 = icmp eq i64 %arg7, %t55
  br i1 %t56, label %.smt_body_8_23, label %.smt_next_8_23
.smt_next_8_23:
  %t57 = add i64 0, 52
  %t58 = icmp eq i64 %arg7, %t57
  br i1 %t58, label %.smt_body_8_24, label %.smt_next_8_24
.smt_next_8_24:
  %t59 = add i64 0, 80
  %t60 = icmp eq i64 %arg7, %t59
  br i1 %t60, label %.smt_body_8_25, label %.smt_next_8_25
.smt_next_8_25:
  %t61 = add i64 0, 81
  %t62 = icmp eq i64 %arg7, %t61
  br i1 %t62, label %.smt_body_8_26, label %.smt_next_8_26
.smt_next_8_26:
  %t63 = add i64 0, 82
  %t64 = icmp eq i64 %arg7, %t63
  br i1 %t64, label %.smt_body_8_27, label %.smt_next_8_27
.smt_next_8_27:
  %t65 = add i64 0, 83
  %t66 = icmp eq i64 %arg7, %t65
  br i1 %t66, label %.smt_body_8_28, label %.smt_next_8_28
.smt_next_8_28:
  %t67 = add i64 0, 84
  %t68 = icmp eq i64 %arg7, %t67
  br i1 %t68, label %.smt_body_8_29, label %.smt_next_8_29
.smt_next_8_29:
  %t69 = add i64 0, 112
  %t70 = icmp eq i64 %arg7, %t69
  br i1 %t70, label %.smt_body_8_30, label %.smt_next_8_30
.smt_next_8_30:
  %t71 = add i64 0, 113
  %t72 = icmp eq i64 %arg7, %t71
  br i1 %t72, label %.smt_body_8_31, label %.smt_next_8_31
.smt_next_8_31:
  %t73 = add i64 0, 144
  %t74 = icmp eq i64 %arg7, %t73
  br i1 %t74, label %.smt_body_8_32, label %.smt_next_8_32
.smt_next_8_32:
  %t75 = add i64 0, 28
  %t76 = icmp eq i64 %arg7, %t75
  br i1 %t76, label %.smt_body_8_33, label %.smt_next_8_33
.smt_next_8_33:
  %t77 = icmp eq i64 0, 0
  br i1 %t77, label %.smt_body_8_34, label %.smt_next_8_34
.smt_next_8_34:
  br label %.smt_end_8
.smt_body_8_0:
  %t80 = add i64 0, 1
  %t78 = add nsw i64 %arg8, %t80
  ret i64 %t78
.smt_body_8_1:
  %t84 = add i64 0, 0
  %t85 = icmp sgt i64 %t6, %t84
  %t82 = zext i1 %t85 to i8
  %t87 = trunc i8 %t82 to i1
  br i1 %t87, label %guard.then86, label %guard.end86
  guard.then86:
  %t90 = add i64 0, 1
  %t88 = sub nsw i64 %t6, %t90
  %t92 = inttoptr i64 %ac0 to ptr
  %t93 = getelementptr i8, ptr %t92, i64 8192
  store i64 %t88, ptr %t93
  br label %guard.end86
  guard.end86:
  %t97 = add i64 0, 1
  %t95 = add nsw i64 %arg8, %t97
  ret i64 %t95
.smt_body_8_2:
  %t101 = add i64 0, 0
  %t102 = icmp sgt i64 %t6, %t101
  %t99 = zext i1 %t102 to i8
  %t104 = trunc i8 %t99 to i1
  br i1 %t104, label %guard.then103, label %guard.end103
  guard.then103:
  %t108 = inttoptr i64 %ac0 to ptr
  %t109 = getelementptr i8, ptr %t108, i64 0
  %t112 = add i64 0, 1
  %t110 = sub nsw i64 %t6, %t112
  %t113 = getelementptr [1024 x i64], ptr %t109, i64 0, i64 %t110
  %t114 = load i64, ptr %t113
  %t117 = inttoptr i64 %ac0 to ptr
  %t118 = getelementptr i8, ptr %t117, i64 0
  %t120 = getelementptr [1024 x i64], ptr %t118, i64 0, i64 %t6
  store i64 %t114, ptr %t120
  %t123 = add i64 0, 1
  %t121 = add nsw i64 %t6, %t123
  %t125 = inttoptr i64 %ac0 to ptr
  %t126 = getelementptr i8, ptr %t125, i64 8192
  store i64 %t121, ptr %t126
  br label %guard.end103
  guard.end103:
  %t130 = add i64 0, 1
  %t128 = add nsw i64 %arg8, %t130
  ret i64 %t128
.smt_body_8_3:
  %t134 = add i64 0, 2
  %t135 = icmp sge i64 %t6, %t134
  %t132 = zext i1 %t135 to i8
  %t137 = trunc i8 %t132 to i1
  br i1 %t137, label %guard.then136, label %guard.end136
  guard.then136:
  %t141 = inttoptr i64 %ac0 to ptr
  %t142 = getelementptr i8, ptr %t141, i64 0
  %t145 = add i64 0, 1
  %t143 = sub nsw i64 %t6, %t145
  %t146 = getelementptr [1024 x i64], ptr %t142, i64 0, i64 %t143
  %t147 = load i64, ptr %t146
  %t151 = inttoptr i64 %ac0 to ptr
  %t152 = getelementptr i8, ptr %t151, i64 0
  %t155 = add i64 0, 2
  %t153 = sub nsw i64 %t6, %t155
  %t156 = getelementptr [1024 x i64], ptr %t152, i64 0, i64 %t153
  %t157 = load i64, ptr %t156
  %t161 = inttoptr i64 %ac0 to ptr
  %t162 = getelementptr i8, ptr %t161, i64 0
  %t165 = add i64 0, 2
  %t163 = sub nsw i64 %t6, %t165
  %t166 = getelementptr [1024 x i64], ptr %t162, i64 0, i64 %t163
  store i64 %t147, ptr %t166
  %t170 = inttoptr i64 %ac0 to ptr
  %t171 = getelementptr i8, ptr %t170, i64 0
  %t174 = add i64 0, 1
  %t172 = sub nsw i64 %t6, %t174
  %t175 = getelementptr [1024 x i64], ptr %t171, i64 0, i64 %t172
  store i64 %t157, ptr %t175
  br label %guard.end136
  guard.end136:
  %t179 = add i64 0, 1
  %t177 = add nsw i64 %arg8, %t179
  ret i64 %t177
.smt_body_8_4:
  %t183 = add i64 0, 2
  %t184 = icmp sge i64 %t6, %t183
  %t181 = zext i1 %t184 to i8
  %t186 = trunc i8 %t181 to i1
  br i1 %t186, label %guard.then185, label %guard.end185
  guard.then185:
  %t191 = inttoptr i64 %ac0 to ptr
  %t192 = getelementptr i8, ptr %t191, i64 0
  %t195 = add i64 0, 2
  %t193 = sub nsw i64 %t6, %t195
  %t196 = getelementptr [1024 x i64], ptr %t192, i64 0, i64 %t193
  %t197 = load i64, ptr %t196
  %t201 = inttoptr i64 %ac0 to ptr
  %t202 = getelementptr i8, ptr %t201, i64 0
  %t205 = add i64 0, 1
  %t203 = sub nsw i64 %t6, %t205
  %t206 = getelementptr [1024 x i64], ptr %t202, i64 0, i64 %t203
  %t207 = load i64, ptr %t206
  %t187 = add nsw i64 %t197, %t207
  %t210 = inttoptr i64 %ac0 to ptr
  %t211 = getelementptr i8, ptr %t210, i64 0
  %t214 = add i64 0, 2
  %t212 = sub nsw i64 %t6, %t214
  %t215 = getelementptr [1024 x i64], ptr %t211, i64 0, i64 %t212
  store i64 %t187, ptr %t215
  %t218 = add i64 0, 1
  %t216 = sub nsw i64 %t6, %t218
  %t220 = inttoptr i64 %ac0 to ptr
  %t221 = getelementptr i8, ptr %t220, i64 8192
  store i64 %t216, ptr %t221
  br label %guard.end185
  guard.end185:
  %t225 = add i64 0, 1
  %t223 = add nsw i64 %arg8, %t225
  ret i64 %t223
.smt_body_8_5:
  %t229 = add i64 0, 2
  %t230 = icmp sge i64 %t6, %t229
  %t227 = zext i1 %t230 to i8
  %t232 = trunc i8 %t227 to i1
  br i1 %t232, label %guard.then231, label %guard.end231
  guard.then231:
  %t237 = inttoptr i64 %ac0 to ptr
  %t238 = getelementptr i8, ptr %t237, i64 0
  %t241 = add i64 0, 2
  %t239 = sub nsw i64 %t6, %t241
  %t242 = getelementptr [1024 x i64], ptr %t238, i64 0, i64 %t239
  %t243 = load i64, ptr %t242
  %t247 = inttoptr i64 %ac0 to ptr
  %t248 = getelementptr i8, ptr %t247, i64 0
  %t251 = add i64 0, 1
  %t249 = sub nsw i64 %t6, %t251
  %t252 = getelementptr [1024 x i64], ptr %t248, i64 0, i64 %t249
  %t253 = load i64, ptr %t252
  %t233 = sub nsw i64 %t243, %t253
  %t256 = inttoptr i64 %ac0 to ptr
  %t257 = getelementptr i8, ptr %t256, i64 0
  %t260 = add i64 0, 2
  %t258 = sub nsw i64 %t6, %t260
  %t261 = getelementptr [1024 x i64], ptr %t257, i64 0, i64 %t258
  store i64 %t233, ptr %t261
  %t264 = add i64 0, 1
  %t262 = sub nsw i64 %t6, %t264
  %t266 = inttoptr i64 %ac0 to ptr
  %t267 = getelementptr i8, ptr %t266, i64 8192
  store i64 %t262, ptr %t267
  br label %guard.end231
  guard.end231:
  %t271 = add i64 0, 1
  %t269 = add nsw i64 %arg8, %t271
  ret i64 %t269
.smt_body_8_6:
  %t275 = add i64 0, 2
  %t276 = icmp sge i64 %t6, %t275
  %t273 = zext i1 %t276 to i8
  %t278 = trunc i8 %t273 to i1
  br i1 %t278, label %guard.then277, label %guard.end277
  guard.then277:
  %t283 = inttoptr i64 %ac0 to ptr
  %t284 = getelementptr i8, ptr %t283, i64 0
  %t287 = add i64 0, 2
  %t285 = sub nsw i64 %t6, %t287
  %t288 = getelementptr [1024 x i64], ptr %t284, i64 0, i64 %t285
  %t289 = load i64, ptr %t288
  %t293 = inttoptr i64 %ac0 to ptr
  %t294 = getelementptr i8, ptr %t293, i64 0
  %t297 = add i64 0, 1
  %t295 = sub nsw i64 %t6, %t297
  %t298 = getelementptr [1024 x i64], ptr %t294, i64 0, i64 %t295
  %t299 = load i64, ptr %t298
  %t279 = mul nsw i64 %t289, %t299
  %t302 = inttoptr i64 %ac0 to ptr
  %t303 = getelementptr i8, ptr %t302, i64 0
  %t306 = add i64 0, 2
  %t304 = sub nsw i64 %t6, %t306
  %t307 = getelementptr [1024 x i64], ptr %t303, i64 0, i64 %t304
  store i64 %t279, ptr %t307
  %t310 = add i64 0, 1
  %t308 = sub nsw i64 %t6, %t310
  %t312 = inttoptr i64 %ac0 to ptr
  %t313 = getelementptr i8, ptr %t312, i64 8192
  store i64 %t308, ptr %t313
  br label %guard.end277
  guard.end277:
  %t317 = add i64 0, 1
  %t315 = add nsw i64 %arg8, %t317
  ret i64 %t315
.smt_body_8_7:
  %t321 = add i64 0, 2
  %t322 = icmp sge i64 %t6, %t321
  %t319 = zext i1 %t322 to i8
  %t324 = trunc i8 %t319 to i1
  br i1 %t324, label %guard.then323, label %guard.end323
  guard.then323:
  %t328 = inttoptr i64 %ac0 to ptr
  %t329 = getelementptr i8, ptr %t328, i64 0
  %t332 = add i64 0, 1
  %t330 = sub nsw i64 %t6, %t332
  %t333 = getelementptr [1024 x i64], ptr %t329, i64 0, i64 %t330
  %t334 = load i64, ptr %t333
  %t338 = inttoptr i64 %ac0 to ptr
  %t339 = getelementptr i8, ptr %t338, i64 0
  %t342 = add i64 0, 2
  %t340 = sub nsw i64 %t6, %t342
  %t343 = getelementptr [1024 x i64], ptr %t339, i64 0, i64 %t340
  %t344 = load i64, ptr %t343
  %t347 = add i64 0, 0
  %t348 = icmp ne i64 %t334, %t347
  %t345 = zext i1 %t348 to i8
  %t350 = trunc i8 %t345 to i1
  br i1 %t350, label %guard.then349, label %guard.end349
  guard.then349:
  %t351 = sdiv i64 %t344, %t334
  %t356 = inttoptr i64 %ac0 to ptr
  %t357 = getelementptr i8, ptr %t356, i64 0
  %t360 = add i64 0, 2
  %t358 = sub nsw i64 %t6, %t360
  %t361 = getelementptr [1024 x i64], ptr %t357, i64 0, i64 %t358
  store i64 %t351, ptr %t361
  %t364 = add i64 0, 1
  %t362 = sub nsw i64 %t6, %t364
  %t366 = inttoptr i64 %ac0 to ptr
  %t367 = getelementptr i8, ptr %t366, i64 8192
  store i64 %t362, ptr %t367
  br label %guard.end349
  guard.end349:
  br label %guard.end323
  guard.end323:
  %t372 = add i64 0, 1
  %t370 = add nsw i64 %arg8, %t372
  ret i64 %t370
.smt_body_8_8:
  %t376 = add i64 0, 2
  %t377 = icmp sge i64 %t6, %t376
  %t374 = zext i1 %t377 to i8
  %t379 = trunc i8 %t374 to i1
  br i1 %t379, label %guard.then378, label %guard.end378
  guard.then378:
  %t383 = inttoptr i64 %ac0 to ptr
  %t384 = getelementptr i8, ptr %t383, i64 0
  %t387 = add i64 0, 1
  %t385 = sub nsw i64 %t6, %t387
  %t388 = getelementptr [1024 x i64], ptr %t384, i64 0, i64 %t385
  %t389 = load i64, ptr %t388
  %t393 = inttoptr i64 %ac0 to ptr
  %t394 = getelementptr i8, ptr %t393, i64 0
  %t397 = add i64 0, 2
  %t395 = sub nsw i64 %t6, %t397
  %t398 = getelementptr [1024 x i64], ptr %t394, i64 0, i64 %t395
  %t399 = load i64, ptr %t398
  %t402 = add i64 0, 0
  %t403 = icmp ne i64 %t389, %t402
  %t400 = zext i1 %t403 to i8
  %t405 = trunc i8 %t400 to i1
  br i1 %t405, label %guard.then404, label %guard.end404
  guard.then404:
  %t406 = srem i64 %t399, %t389
  %t411 = inttoptr i64 %ac0 to ptr
  %t412 = getelementptr i8, ptr %t411, i64 0
  %t415 = add i64 0, 2
  %t413 = sub nsw i64 %t6, %t415
  %t416 = getelementptr [1024 x i64], ptr %t412, i64 0, i64 %t413
  store i64 %t406, ptr %t416
  %t419 = add i64 0, 1
  %t417 = sub nsw i64 %t6, %t419
  %t421 = inttoptr i64 %ac0 to ptr
  %t422 = getelementptr i8, ptr %t421, i64 8192
  store i64 %t417, ptr %t422
  br label %guard.end404
  guard.end404:
  br label %guard.end378
  guard.end378:
  %t427 = add i64 0, 1
  %t425 = add nsw i64 %arg8, %t427
  ret i64 %t425
.smt_body_8_9:
  %t431 = add i64 0, 2
  %t432 = icmp sge i64 %t6, %t431
  %t429 = zext i1 %t432 to i8
  %t434 = trunc i8 %t429 to i1
  br i1 %t434, label %guard.then433, label %guard.end433
  guard.then433:
  %t439 = inttoptr i64 %ac0 to ptr
  %t440 = getelementptr i8, ptr %t439, i64 0
  %t443 = add i64 0, 2
  %t441 = sub nsw i64 %t6, %t443
  %t444 = getelementptr [1024 x i64], ptr %t440, i64 0, i64 %t441
  %t445 = load i64, ptr %t444
  %t449 = inttoptr i64 %ac0 to ptr
  %t450 = getelementptr i8, ptr %t449, i64 0
  %t453 = add i64 0, 1
  %t451 = sub nsw i64 %t6, %t453
  %t454 = getelementptr [1024 x i64], ptr %t450, i64 0, i64 %t451
  %t455 = load i64, ptr %t454
  %t435 = and i64 %t445, %t455
  %t458 = inttoptr i64 %ac0 to ptr
  %t459 = getelementptr i8, ptr %t458, i64 0
  %t462 = add i64 0, 2
  %t460 = sub nsw i64 %t6, %t462
  %t463 = getelementptr [1024 x i64], ptr %t459, i64 0, i64 %t460
  store i64 %t435, ptr %t463
  %t466 = add i64 0, 1
  %t464 = sub nsw i64 %t6, %t466
  %t468 = inttoptr i64 %ac0 to ptr
  %t469 = getelementptr i8, ptr %t468, i64 8192
  store i64 %t464, ptr %t469
  br label %guard.end433
  guard.end433:
  %t473 = add i64 0, 1
  %t471 = add nsw i64 %arg8, %t473
  ret i64 %t471
.smt_body_8_10:
  %t477 = add i64 0, 2
  %t478 = icmp sge i64 %t6, %t477
  %t475 = zext i1 %t478 to i8
  %t480 = trunc i8 %t475 to i1
  br i1 %t480, label %guard.then479, label %guard.end479
  guard.then479:
  %t485 = inttoptr i64 %ac0 to ptr
  %t486 = getelementptr i8, ptr %t485, i64 0
  %t489 = add i64 0, 2
  %t487 = sub nsw i64 %t6, %t489
  %t490 = getelementptr [1024 x i64], ptr %t486, i64 0, i64 %t487
  %t491 = load i64, ptr %t490
  %t495 = inttoptr i64 %ac0 to ptr
  %t496 = getelementptr i8, ptr %t495, i64 0
  %t499 = add i64 0, 1
  %t497 = sub nsw i64 %t6, %t499
  %t500 = getelementptr [1024 x i64], ptr %t496, i64 0, i64 %t497
  %t501 = load i64, ptr %t500
  %t481 = or i64 %t491, %t501
  %t504 = inttoptr i64 %ac0 to ptr
  %t505 = getelementptr i8, ptr %t504, i64 0
  %t508 = add i64 0, 2
  %t506 = sub nsw i64 %t6, %t508
  %t509 = getelementptr [1024 x i64], ptr %t505, i64 0, i64 %t506
  store i64 %t481, ptr %t509
  %t512 = add i64 0, 1
  %t510 = sub nsw i64 %t6, %t512
  %t514 = inttoptr i64 %ac0 to ptr
  %t515 = getelementptr i8, ptr %t514, i64 8192
  store i64 %t510, ptr %t515
  br label %guard.end479
  guard.end479:
  %t519 = add i64 0, 1
  %t517 = add nsw i64 %arg8, %t519
  ret i64 %t517
.smt_body_8_11:
  %t523 = add i64 0, 2
  %t524 = icmp sge i64 %t6, %t523
  %t521 = zext i1 %t524 to i8
  %t526 = trunc i8 %t521 to i1
  br i1 %t526, label %guard.then525, label %guard.end525
  guard.then525:
  %t531 = inttoptr i64 %ac0 to ptr
  %t532 = getelementptr i8, ptr %t531, i64 0
  %t535 = add i64 0, 2
  %t533 = sub nsw i64 %t6, %t535
  %t536 = getelementptr [1024 x i64], ptr %t532, i64 0, i64 %t533
  %t537 = load i64, ptr %t536
  %t541 = inttoptr i64 %ac0 to ptr
  %t542 = getelementptr i8, ptr %t541, i64 0
  %t545 = add i64 0, 1
  %t543 = sub nsw i64 %t6, %t545
  %t546 = getelementptr [1024 x i64], ptr %t542, i64 0, i64 %t543
  %t547 = load i64, ptr %t546
  %t527 = xor i64 %t537, %t547
  %t550 = inttoptr i64 %ac0 to ptr
  %t551 = getelementptr i8, ptr %t550, i64 0
  %t554 = add i64 0, 2
  %t552 = sub nsw i64 %t6, %t554
  %t555 = getelementptr [1024 x i64], ptr %t551, i64 0, i64 %t552
  store i64 %t527, ptr %t555
  %t558 = add i64 0, 1
  %t556 = sub nsw i64 %t6, %t558
  %t560 = inttoptr i64 %ac0 to ptr
  %t561 = getelementptr i8, ptr %t560, i64 8192
  store i64 %t556, ptr %t561
  br label %guard.end525
  guard.end525:
  %t565 = add i64 0, 1
  %t563 = add nsw i64 %arg8, %t565
  ret i64 %t563
.smt_body_8_12:
  %t569 = add i64 0, 1
  %t570 = icmp sge i64 %t6, %t569
  %t567 = zext i1 %t570 to i8
  %t572 = trunc i8 %t567 to i1
  br i1 %t572, label %guard.then571, label %guard.end571
  guard.then571:
  %t577 = inttoptr i64 %ac0 to ptr
  %t578 = getelementptr i8, ptr %t577, i64 0
  %t581 = add i64 0, 1
  %t579 = sub nsw i64 %t6, %t581
  %t582 = getelementptr [1024 x i64], ptr %t578, i64 0, i64 %t579
  %t583 = load i64, ptr %t582
  %t584 = add i64 0, 0
  %t585 = icmp eq i64 %t583, %t584
  %t573 = zext i1 %t585 to i8
  %t587 = trunc i8 %t573 to i1
  br i1 %t587, label %guard.then586, label %guard.end586
  guard.then586:
  %t588 = add i64 0, 1
  %t591 = inttoptr i64 %ac0 to ptr
  %t592 = getelementptr i8, ptr %t591, i64 0
  %t595 = add i64 0, 1
  %t593 = sub nsw i64 %t6, %t595
  %t596 = getelementptr [1024 x i64], ptr %t592, i64 0, i64 %t593
  store i64 %t588, ptr %t596
  br label %guard.end586
  guard.end586:
  %t602 = inttoptr i64 %ac0 to ptr
  %t603 = getelementptr i8, ptr %t602, i64 0
  %t606 = add i64 0, 1
  %t604 = sub nsw i64 %t6, %t606
  %t607 = getelementptr [1024 x i64], ptr %t603, i64 0, i64 %t604
  %t608 = load i64, ptr %t607
  %t609 = add i64 0, 0
  %t610 = icmp ne i64 %t608, %t609
  %t598 = zext i1 %t610 to i8
  %t612 = trunc i8 %t598 to i1
  br i1 %t612, label %guard.then611, label %guard.end611
  guard.then611:
  %t613 = add i64 0, 0
  %t616 = inttoptr i64 %ac0 to ptr
  %t617 = getelementptr i8, ptr %t616, i64 0
  %t620 = add i64 0, 1
  %t618 = sub nsw i64 %t6, %t620
  %t621 = getelementptr [1024 x i64], ptr %t617, i64 0, i64 %t618
  store i64 %t613, ptr %t621
  br label %guard.end611
  guard.end611:
  br label %guard.end571
  guard.end571:
  %t626 = add i64 0, 1
  %t624 = add nsw i64 %arg8, %t626
  ret i64 %t624
.smt_body_8_13:
  %t630 = add i64 0, 2
  %t631 = icmp sge i64 %t6, %t630
  %t628 = zext i1 %t631 to i8
  %t633 = trunc i8 %t628 to i1
  br i1 %t633, label %guard.then632, label %guard.end632
  guard.then632:
  %t638 = inttoptr i64 %ac0 to ptr
  %t639 = getelementptr i8, ptr %t638, i64 0
  %t642 = add i64 0, 2
  %t640 = sub nsw i64 %t6, %t642
  %t643 = getelementptr [1024 x i64], ptr %t639, i64 0, i64 %t640
  %t644 = load i64, ptr %t643
  %t649 = inttoptr i64 %ac0 to ptr
  %t650 = getelementptr i8, ptr %t649, i64 0
  %t653 = add i64 0, 1
  %t651 = sub nsw i64 %t6, %t653
  %t654 = getelementptr [1024 x i64], ptr %t650, i64 0, i64 %t651
  %t655 = load i64, ptr %t654
  %t656 = add i64 0, 63
  %t645 = and i64 %t655, %t656
  %t634 = shl i64 %t644, %t645
  %t659 = inttoptr i64 %ac0 to ptr
  %t660 = getelementptr i8, ptr %t659, i64 0
  %t663 = add i64 0, 2
  %t661 = sub nsw i64 %t6, %t663
  %t664 = getelementptr [1024 x i64], ptr %t660, i64 0, i64 %t661
  store i64 %t634, ptr %t664
  %t667 = add i64 0, 1
  %t665 = sub nsw i64 %t6, %t667
  %t669 = inttoptr i64 %ac0 to ptr
  %t670 = getelementptr i8, ptr %t669, i64 8192
  store i64 %t665, ptr %t670
  br label %guard.end632
  guard.end632:
  %t674 = add i64 0, 1
  %t672 = add nsw i64 %arg8, %t674
  ret i64 %t672
.smt_body_8_14:
  %t678 = add i64 0, 2
  %t679 = icmp sge i64 %t6, %t678
  %t676 = zext i1 %t679 to i8
  %t681 = trunc i8 %t676 to i1
  br i1 %t681, label %guard.then680, label %guard.end680
  guard.then680:
  %t686 = inttoptr i64 %ac0 to ptr
  %t687 = getelementptr i8, ptr %t686, i64 0
  %t690 = add i64 0, 2
  %t688 = sub nsw i64 %t6, %t690
  %t691 = getelementptr [1024 x i64], ptr %t687, i64 0, i64 %t688
  %t692 = load i64, ptr %t691
  %t697 = inttoptr i64 %ac0 to ptr
  %t698 = getelementptr i8, ptr %t697, i64 0
  %t701 = add i64 0, 1
  %t699 = sub nsw i64 %t6, %t701
  %t702 = getelementptr [1024 x i64], ptr %t698, i64 0, i64 %t699
  %t703 = load i64, ptr %t702
  %t704 = add i64 0, 63
  %t693 = and i64 %t703, %t704
  %t682 = ashr i64 %t692, %t693
  %t707 = inttoptr i64 %ac0 to ptr
  %t708 = getelementptr i8, ptr %t707, i64 0
  %t711 = add i64 0, 2
  %t709 = sub nsw i64 %t6, %t711
  %t712 = getelementptr [1024 x i64], ptr %t708, i64 0, i64 %t709
  store i64 %t682, ptr %t712
  %t715 = add i64 0, 1
  %t713 = sub nsw i64 %t6, %t715
  %t717 = inttoptr i64 %ac0 to ptr
  %t718 = getelementptr i8, ptr %t717, i64 8192
  store i64 %t713, ptr %t718
  br label %guard.end680
  guard.end680:
  %t722 = add i64 0, 1
  %t720 = add nsw i64 %arg8, %t722
  ret i64 %t720
.smt_body_8_15:
  %t726 = add i64 0, 2
  %t727 = icmp sge i64 %t6, %t726
  %t724 = zext i1 %t727 to i8
  %t729 = trunc i8 %t724 to i1
  br i1 %t729, label %guard.then728, label %guard.end728
  guard.then728:
  %t734 = inttoptr i64 %ac0 to ptr
  %t735 = getelementptr i8, ptr %t734, i64 0
  %t738 = add i64 0, 2
  %t736 = sub nsw i64 %t6, %t738
  %t739 = getelementptr [1024 x i64], ptr %t735, i64 0, i64 %t736
  %t740 = load i64, ptr %t739
  %t744 = inttoptr i64 %ac0 to ptr
  %t745 = getelementptr i8, ptr %t744, i64 0
  %t748 = add i64 0, 1
  %t746 = sub nsw i64 %t6, %t748
  %t749 = getelementptr [1024 x i64], ptr %t745, i64 0, i64 %t746
  %t750 = load i64, ptr %t749
  %t751 = icmp eq i64 %t740, %t750
  %t730 = zext i1 %t751 to i8
  %t753 = trunc i8 %t730 to i1
  br i1 %t753, label %guard.then752, label %guard.end752
  guard.then752:
  %t754 = add i64 0, 1
  %t757 = inttoptr i64 %ac0 to ptr
  %t758 = getelementptr i8, ptr %t757, i64 0
  %t761 = add i64 0, 2
  %t759 = sub nsw i64 %t6, %t761
  %t762 = getelementptr [1024 x i64], ptr %t758, i64 0, i64 %t759
  store i64 %t754, ptr %t762
  br label %guard.end752
  guard.end752:
  %t768 = inttoptr i64 %ac0 to ptr
  %t769 = getelementptr i8, ptr %t768, i64 0
  %t772 = add i64 0, 2
  %t770 = sub nsw i64 %t6, %t772
  %t773 = getelementptr [1024 x i64], ptr %t769, i64 0, i64 %t770
  %t774 = load i64, ptr %t773
  %t778 = inttoptr i64 %ac0 to ptr
  %t779 = getelementptr i8, ptr %t778, i64 0
  %t782 = add i64 0, 1
  %t780 = sub nsw i64 %t6, %t782
  %t783 = getelementptr [1024 x i64], ptr %t779, i64 0, i64 %t780
  %t784 = load i64, ptr %t783
  %t785 = icmp ne i64 %t774, %t784
  %t764 = zext i1 %t785 to i8
  %t787 = trunc i8 %t764 to i1
  br i1 %t787, label %guard.then786, label %guard.end786
  guard.then786:
  %t788 = add i64 0, 0
  %t791 = inttoptr i64 %ac0 to ptr
  %t792 = getelementptr i8, ptr %t791, i64 0
  %t795 = add i64 0, 2
  %t793 = sub nsw i64 %t6, %t795
  %t796 = getelementptr [1024 x i64], ptr %t792, i64 0, i64 %t793
  store i64 %t788, ptr %t796
  br label %guard.end786
  guard.end786:
  %t800 = add i64 0, 1
  %t798 = sub nsw i64 %t6, %t800
  %t802 = inttoptr i64 %ac0 to ptr
  %t803 = getelementptr i8, ptr %t802, i64 8192
  store i64 %t798, ptr %t803
  br label %guard.end728
  guard.end728:
  %t807 = add i64 0, 1
  %t805 = add nsw i64 %arg8, %t807
  ret i64 %t805
.smt_body_8_16:
  %t811 = add i64 0, 1
  %t812 = icmp sge i64 %t6, %t811
  %t809 = zext i1 %t812 to i8
  %t814 = trunc i8 %t809 to i1
  br i1 %t814, label %guard.then813, label %guard.end813
  guard.then813:
  %t818 = inttoptr i64 %ac0 to ptr
  %t819 = getelementptr i8, ptr %t818, i64 0
  %t822 = add i64 0, 1
  %t820 = sub nsw i64 %t6, %t822
  %t823 = getelementptr [1024 x i64], ptr %t819, i64 0, i64 %t820
  %t824 = load i64, ptr %t823
  %t827 = inttoptr i64 %t824 to ptr
  %t825 = load i64, ptr %t827
  %t830 = inttoptr i64 %ac0 to ptr
  %t831 = getelementptr i8, ptr %t830, i64 0
  %t834 = add i64 0, 1
  %t832 = sub nsw i64 %t6, %t834
  %t835 = getelementptr [1024 x i64], ptr %t831, i64 0, i64 %t832
  store i64 %t825, ptr %t835
  br label %guard.end813
  guard.end813:
  %t839 = add i64 0, 1
  %t837 = add nsw i64 %arg8, %t839
  ret i64 %t837
.smt_body_8_17:
  %t843 = add i64 0, 2
  %t844 = icmp sge i64 %t6, %t843
  %t841 = zext i1 %t844 to i8
  %t846 = trunc i8 %t841 to i1
  br i1 %t846, label %guard.then845, label %guard.end845
  guard.then845:
  %t850 = inttoptr i64 %ac0 to ptr
  %t851 = getelementptr i8, ptr %t850, i64 0
  %t854 = add i64 0, 2
  %t852 = sub nsw i64 %t6, %t854
  %t855 = getelementptr [1024 x i64], ptr %t851, i64 0, i64 %t852
  %t856 = load i64, ptr %t855
  %t860 = inttoptr i64 %ac0 to ptr
  %t861 = getelementptr i8, ptr %t860, i64 0
  %t864 = add i64 0, 1
  %t862 = sub nsw i64 %t6, %t864
  %t865 = getelementptr [1024 x i64], ptr %t861, i64 0, i64 %t862
  %t866 = load i64, ptr %t865
  %t870 = inttoptr i64 %t856 to ptr
  store i64 %t866, ptr %t870
  %t867 = add i64 0, 0
  %t873 = add i64 0, 2
  %t871 = sub nsw i64 %t6, %t873
  %t875 = inttoptr i64 %ac0 to ptr
  %t876 = getelementptr i8, ptr %t875, i64 8192
  store i64 %t871, ptr %t876
  br label %guard.end845
  guard.end845:
  %t880 = add i64 0, 1
  %t878 = add nsw i64 %arg8, %t880
  ret i64 %t878
.smt_body_8_18:
  %t883 = add i64 0, 1
  %t882 = sub i64 0, %t883
  ret i64 %t882
.smt_body_8_19:
  %t886 = add i64 0, 1
  %t885 = sub i64 0, %t886
  ret i64 %t885
.smt_body_8_20:
  %t892 = add i64 0, 1
  %t890 = add nsw i64 %arg8, %t892
  %t893 = inttoptr i64 %ac3 to ptr
  %t888 = call i64 @read_i8(ptr %state, ptr %t893, i64 %t890)
  %t896 = add i64 0, 1024
  %t897 = icmp slt i64 %t6, %t896
  %t894 = zext i1 %t897 to i8
  %t899 = trunc i8 %t894 to i1
  br i1 %t899, label %guard.then898, label %guard.end898
  guard.then898:
  %t903 = inttoptr i64 %ac0 to ptr
  %t904 = getelementptr i8, ptr %t903, i64 0
  %t906 = getelementptr [1024 x i64], ptr %t904, i64 0, i64 %t6
  store i64 %t888, ptr %t906
  %t909 = add i64 0, 1
  %t907 = add nsw i64 %t6, %t909
  %t911 = inttoptr i64 %ac0 to ptr
  %t912 = getelementptr i8, ptr %t911, i64 8192
  store i64 %t907, ptr %t912
  br label %guard.end898
  guard.end898:
  %t916 = add i64 0, 2
  %t914 = add nsw i64 %arg8, %t916
  ret i64 %t914
.smt_body_8_21:
  %t922 = add i64 0, 1
  %t920 = add nsw i64 %arg8, %t922
  %t923 = inttoptr i64 %ac3 to ptr
  %t918 = call i64 @read_u8(ptr %state, ptr %t923, i64 %t920)
  %t927 = inttoptr i64 %ac1 to ptr
  %t928 = getelementptr i8, ptr %t927, i64 0
  %t929 = add nsw i64 %arg9, %t918
  %t932 = getelementptr [4096 x i64], ptr %t928, i64 0, i64 %t929
  %t933 = load i64, ptr %t932
  %t936 = add i64 0, 1024
  %t937 = icmp slt i64 %t6, %t936
  %t934 = zext i1 %t937 to i8
  %t939 = trunc i8 %t934 to i1
  br i1 %t939, label %guard.then938, label %guard.end938
  guard.then938:
  %t943 = inttoptr i64 %ac0 to ptr
  %t944 = getelementptr i8, ptr %t943, i64 0
  %t946 = getelementptr [1024 x i64], ptr %t944, i64 0, i64 %t6
  store i64 %t933, ptr %t946
  %t949 = add i64 0, 1
  %t947 = add nsw i64 %t6, %t949
  %t951 = inttoptr i64 %ac0 to ptr
  %t952 = getelementptr i8, ptr %t951, i64 8192
  store i64 %t947, ptr %t952
  br label %guard.end938
  guard.end938:
  %t956 = add i64 0, 2
  %t954 = add nsw i64 %arg8, %t956
  ret i64 %t954
.smt_body_8_22:
  %t962 = add i64 0, 1
  %t960 = add nsw i64 %arg8, %t962
  %t963 = inttoptr i64 %ac3 to ptr
  %t958 = call i64 @read_u8(ptr %state, ptr %t963, i64 %t960)
  %t966 = add i64 0, 1
  %t967 = icmp sge i64 %t6, %t966
  %t964 = zext i1 %t967 to i8
  %t969 = trunc i8 %t964 to i1
  br i1 %t969, label %guard.then968, label %guard.end968
  guard.then968:
  %t973 = inttoptr i64 %ac0 to ptr
  %t974 = getelementptr i8, ptr %t973, i64 0
  %t977 = add i64 0, 1
  %t975 = sub nsw i64 %t6, %t977
  %t978 = getelementptr [1024 x i64], ptr %t974, i64 0, i64 %t975
  %t979 = load i64, ptr %t978
  %t982 = inttoptr i64 %ac1 to ptr
  %t983 = getelementptr i8, ptr %t982, i64 0
  %t984 = add nsw i64 %arg9, %t958
  %t987 = getelementptr [4096 x i64], ptr %t983, i64 0, i64 %t984
  store i64 %t979, ptr %t987
  %t990 = add i64 0, 1
  %t988 = sub nsw i64 %t6, %t990
  %t992 = inttoptr i64 %ac0 to ptr
  %t993 = getelementptr i8, ptr %t992, i64 8192
  store i64 %t988, ptr %t993
  br label %guard.end968
  guard.end968:
  %t997 = add i64 0, 2
  %t995 = add nsw i64 %arg8, %t997
  ret i64 %t995
.smt_body_8_23:
  %t1003 = add i64 0, 1
  %t1001 = add nsw i64 %arg8, %t1003
  %t1004 = inttoptr i64 %ac3 to ptr
  %t999 = call i64 @read_u8(ptr %state, ptr %t1004, i64 %t1001)
    %t1010 = inttoptr i64 %ac2 to ptr
    %t1011 = getelementptr i8, ptr %t1010, i64 6144
    %t1012 = load i64, ptr %t1011
  %t1013 = add i64 0, 256
  %t1014 = icmp slt i64 %t1012, %t1013
  %t1005 = zext i1 %t1014 to i8
  %t1016 = trunc i8 %t1005 to i1
  br i1 %t1016, label %guard.then1015, label %guard.end1015
  guard.then1015:
    %t1021 = inttoptr i64 %ac1 to ptr
    %t1022 = getelementptr i8, ptr %t1021, i64 32768
    %t1023 = load i64, ptr %t1022
  %t1027 = inttoptr i64 %ac2 to ptr
  %t1028 = getelementptr i8, ptr %t1027, i64 0
    %t1033 = inttoptr i64 %ac2 to ptr
    %t1034 = getelementptr i8, ptr %t1033, i64 6144
    %t1035 = load i64, ptr %t1034
  %t1036 = mul i64 %t1035, 24
  %t1037 = getelementptr i8, ptr %t1028, i64 %t1036
  %t1038 = ptrtoint ptr %t1037 to i64
  %t1039 = inttoptr i64 %t1038 to ptr
  %t1040 = getelementptr i8, ptr %t1039, i64 0
  store i64 %t1023, ptr %t1040
  %t1045 = inttoptr i64 %ac2 to ptr
  %t1046 = getelementptr i8, ptr %t1045, i64 0
    %t1051 = inttoptr i64 %ac2 to ptr
    %t1052 = getelementptr i8, ptr %t1051, i64 6144
    %t1053 = load i64, ptr %t1052
  %t1054 = mul i64 %t1053, 24
  %t1055 = getelementptr i8, ptr %t1046, i64 %t1054
  %t1056 = ptrtoint ptr %t1055 to i64
  %t1057 = inttoptr i64 %t1056 to ptr
  %t1058 = getelementptr i8, ptr %t1057, i64 8
  store i64 %t999, ptr %t1058
  %t1061 = add i64 0, 2
  %t1059 = add nsw i64 %arg8, %t1061
  %t1065 = inttoptr i64 %ac2 to ptr
  %t1066 = getelementptr i8, ptr %t1065, i64 0
    %t1071 = inttoptr i64 %ac2 to ptr
    %t1072 = getelementptr i8, ptr %t1071, i64 6144
    %t1073 = load i64, ptr %t1072
  %t1074 = mul i64 %t1073, 24
  %t1075 = getelementptr i8, ptr %t1066, i64 %t1074
  %t1076 = ptrtoint ptr %t1075 to i64
  %t1077 = inttoptr i64 %t1076 to ptr
  %t1078 = getelementptr i8, ptr %t1077, i64 16
  store i64 %t1059, ptr %t1078
    %t1084 = inttoptr i64 %ac2 to ptr
    %t1085 = getelementptr i8, ptr %t1084, i64 6144
    %t1086 = load i64, ptr %t1085
  %t1087 = add i64 0, 1
  %t1079 = add nsw i64 %t1086, %t1087
  %t1089 = inttoptr i64 %ac2 to ptr
  %t1090 = getelementptr i8, ptr %t1089, i64 6144
  store i64 %t1079, ptr %t1090
  br label %guard.end1015
  guard.end1015:
  %t1094 = add i64 0, 2
  %t1092 = add nsw i64 %arg8, %t1094
  ret i64 %t1092
.smt_body_8_24:
    %t1101 = inttoptr i64 %ac2 to ptr
    %t1102 = getelementptr i8, ptr %t1101, i64 6144
    %t1103 = load i64, ptr %t1102
  %t1104 = add i64 0, 0
  %t1105 = icmp sgt i64 %t1103, %t1104
  %t1096 = zext i1 %t1105 to i8
  %t1107 = trunc i8 %t1096 to i1
  br i1 %t1107, label %guard.then1106, label %guard.end1106
  guard.then1106:
    %t1113 = inttoptr i64 %ac2 to ptr
    %t1114 = getelementptr i8, ptr %t1113, i64 6144
    %t1115 = load i64, ptr %t1114
  %t1116 = add i64 0, 1
  %t1108 = sub nsw i64 %t1115, %t1116
  %t1118 = inttoptr i64 %ac2 to ptr
  %t1119 = getelementptr i8, ptr %t1118, i64 6144
  store i64 %t1108, ptr %t1119
  br label %guard.end1106
  guard.end1106:
  %t1123 = add i64 0, 1
  %t1121 = add nsw i64 %arg8, %t1123
  ret i64 %t1121
.smt_body_8_25:
  %t1129 = add i64 0, 1
  %t1127 = add nsw i64 %arg8, %t1129
  %t1130 = inttoptr i64 %ac3 to ptr
  %t1125 = call i64 @read_i16(ptr %state, ptr %t1130, i64 %t1127)
  %t1133 = add i64 0, 1024
  %t1134 = icmp slt i64 %t6, %t1133
  %t1131 = zext i1 %t1134 to i8
  %t1136 = trunc i8 %t1131 to i1
  br i1 %t1136, label %guard.then1135, label %guard.end1135
  guard.then1135:
  %t1140 = inttoptr i64 %ac0 to ptr
  %t1141 = getelementptr i8, ptr %t1140, i64 0
  %t1143 = getelementptr [1024 x i64], ptr %t1141, i64 0, i64 %t6
  store i64 %t1125, ptr %t1143
  %t1146 = add i64 0, 1
  %t1144 = add nsw i64 %t6, %t1146
  %t1148 = inttoptr i64 %ac0 to ptr
  %t1149 = getelementptr i8, ptr %t1148, i64 8192
  store i64 %t1144, ptr %t1149
  br label %guard.end1135
  guard.end1135:
  %t1153 = add i64 0, 3
  %t1151 = add nsw i64 %arg8, %t1153
  ret i64 %t1151
.smt_body_8_26:
  %t1159 = add i64 0, 1
  %t1157 = add nsw i64 %arg8, %t1159
  %t1160 = inttoptr i64 %ac3 to ptr
  %t1155 = call i64 @read_i16(ptr %state, ptr %t1160, i64 %t1157)
  %t1164 = add i64 0, 3
  %t1162 = add nsw i64 %arg8, %t1164
  %t1161 = add nsw i64 %t1162, %t1155
  ret i64 %t1161
.smt_body_8_27:
  %t1171 = add i64 0, 1
  %t1169 = add nsw i64 %arg8, %t1171
  %t1172 = inttoptr i64 %ac3 to ptr
  %t1167 = call i64 @read_i16(ptr %state, ptr %t1172, i64 %t1169)
  %t1175 = add i64 0, 1
  %t1176 = icmp sge i64 %t6, %t1175
  %t1173 = zext i1 %t1176 to i8
  %t1178 = trunc i8 %t1173 to i1
  br i1 %t1178, label %guard.then1177, label %guard.end1177
  guard.then1177:
  %t1182 = inttoptr i64 %ac0 to ptr
  %t1183 = getelementptr i8, ptr %t1182, i64 0
  %t1186 = add i64 0, 1
  %t1184 = sub nsw i64 %t6, %t1186
  %t1187 = getelementptr [1024 x i64], ptr %t1183, i64 0, i64 %t1184
  %t1188 = load i64, ptr %t1187
  %t1191 = add i64 0, 1
  %t1189 = sub nsw i64 %t6, %t1191
  %t1193 = inttoptr i64 %ac0 to ptr
  %t1194 = getelementptr i8, ptr %t1193, i64 8192
  store i64 %t1189, ptr %t1194
  %t1197 = add i64 0, 0
  %t1198 = icmp eq i64 %t1188, %t1197
  %t1195 = zext i1 %t1198 to i8
  %t1200 = trunc i8 %t1195 to i1
  br i1 %t1200, label %guard.then1199, label %guard.end1199
  guard.then1199:
  %t1204 = add i64 0, 3
  %t1202 = add nsw i64 %arg8, %t1204
  %t1201 = add nsw i64 %t1202, %t1167
  ret i64 %t1201
  guard.end1199:
  %t1210 = add i64 0, 3
  %t1208 = add nsw i64 %arg8, %t1210
  ret i64 %t1208
  guard.end1177:
  %t1215 = add i64 0, 3
  %t1213 = add nsw i64 %arg8, %t1215
  ret i64 %t1213
.smt_body_8_28:
  %t1221 = add i64 0, 1
  %t1219 = add nsw i64 %arg8, %t1221
  %t1222 = inttoptr i64 %ac3 to ptr
  %t1217 = call i64 @read_i16(ptr %state, ptr %t1222, i64 %t1219)
  %t1225 = add i64 0, 1
  %t1226 = icmp sge i64 %t6, %t1225
  %t1223 = zext i1 %t1226 to i8
  %t1228 = trunc i8 %t1223 to i1
  br i1 %t1228, label %guard.then1227, label %guard.end1227
  guard.then1227:
  %t1232 = inttoptr i64 %ac0 to ptr
  %t1233 = getelementptr i8, ptr %t1232, i64 0
  %t1236 = add i64 0, 1
  %t1234 = sub nsw i64 %t6, %t1236
  %t1237 = getelementptr [1024 x i64], ptr %t1233, i64 0, i64 %t1234
  %t1238 = load i64, ptr %t1237
  %t1241 = add i64 0, 1
  %t1239 = sub nsw i64 %t6, %t1241
  %t1243 = inttoptr i64 %ac0 to ptr
  %t1244 = getelementptr i8, ptr %t1243, i64 8192
  store i64 %t1239, ptr %t1244
  %t1247 = add i64 0, 0
  %t1248 = icmp ne i64 %t1238, %t1247
  %t1245 = zext i1 %t1248 to i8
  %t1250 = trunc i8 %t1245 to i1
  br i1 %t1250, label %guard.then1249, label %guard.end1249
  guard.then1249:
  %t1254 = add i64 0, 3
  %t1252 = add nsw i64 %arg8, %t1254
  %t1251 = add nsw i64 %t1252, %t1217
  ret i64 %t1251
  guard.end1249:
  %t1260 = add i64 0, 3
  %t1258 = add nsw i64 %arg8, %t1260
  ret i64 %t1258
  guard.end1227:
  %t1265 = add i64 0, 3
  %t1263 = add nsw i64 %arg8, %t1265
  ret i64 %t1263
.smt_body_8_29:
  %t1271 = add i64 0, 1
  %t1269 = add nsw i64 %arg8, %t1271
  %t1272 = inttoptr i64 %ac3 to ptr
  %t1267 = call i64 @read_u16(ptr %state, ptr %t1272, i64 %t1269)
  %t1276 = icmp slt i64 %t1267, %arg6
  %t1273 = zext i1 %t1276 to i8
  %t1278 = trunc i8 %t1273 to i1
  br i1 %t1278, label %guard.then1277, label %guard.end1277
  guard.then1277:
  %t1283 = inttoptr i64 %ac4 to ptr
  %t1279 = call i64 @fn_bc_offset(ptr %state, ptr %t1283, i64 %arg5, i64 %t1267)
  %t1288 = inttoptr i64 %ac4 to ptr
  %t1284 = call i64 @fn_local_count(ptr %state, ptr %t1288, i64 %arg5, i64 %t1267)
  %t1293 = inttoptr i64 %ac4 to ptr
  %t1289 = call i64 @fn_arg_count(ptr %state, ptr %t1293, i64 %arg5, i64 %t1267)
    %t1299 = inttoptr i64 %ac2 to ptr
    %t1300 = getelementptr i8, ptr %t1299, i64 6144
    %t1301 = load i64, ptr %t1300
  %t1302 = add i64 0, 256
  %t1303 = icmp slt i64 %t1301, %t1302
  %t1294 = zext i1 %t1303 to i8
  %t1305 = trunc i8 %t1294 to i1
  br i1 %t1305, label %guard.then1304, label %guard.end1304
  guard.then1304:
  %t1308 = add i64 0, 3
  %t1306 = add nsw i64 %arg8, %t1308
  %t1312 = inttoptr i64 %ac2 to ptr
  %t1313 = getelementptr i8, ptr %t1312, i64 0
    %t1318 = inttoptr i64 %ac2 to ptr
    %t1319 = getelementptr i8, ptr %t1318, i64 6144
    %t1320 = load i64, ptr %t1319
  %t1321 = mul i64 %t1320, 24
  %t1322 = getelementptr i8, ptr %t1313, i64 %t1321
  %t1323 = ptrtoint ptr %t1322 to i64
  %t1324 = inttoptr i64 %t1323 to ptr
  %t1325 = getelementptr i8, ptr %t1324, i64 16
  store i64 %t1306, ptr %t1325
    %t1330 = inttoptr i64 %ac1 to ptr
    %t1331 = getelementptr i8, ptr %t1330, i64 32768
    %t1332 = load i64, ptr %t1331
  %t1336 = inttoptr i64 %ac2 to ptr
  %t1337 = getelementptr i8, ptr %t1336, i64 0
    %t1342 = inttoptr i64 %ac2 to ptr
    %t1343 = getelementptr i8, ptr %t1342, i64 6144
    %t1344 = load i64, ptr %t1343
  %t1345 = mul i64 %t1344, 24
  %t1346 = getelementptr i8, ptr %t1337, i64 %t1345
  %t1347 = ptrtoint ptr %t1346 to i64
  %t1348 = inttoptr i64 %t1347 to ptr
  %t1349 = getelementptr i8, ptr %t1348, i64 0
  store i64 %t1332, ptr %t1349
  %t1354 = inttoptr i64 %ac2 to ptr
  %t1355 = getelementptr i8, ptr %t1354, i64 0
    %t1360 = inttoptr i64 %ac2 to ptr
    %t1361 = getelementptr i8, ptr %t1360, i64 6144
    %t1362 = load i64, ptr %t1361
  %t1363 = mul i64 %t1362, 24
  %t1364 = getelementptr i8, ptr %t1355, i64 %t1363
  %t1365 = ptrtoint ptr %t1364 to i64
  %t1366 = inttoptr i64 %t1365 to ptr
  %t1367 = getelementptr i8, ptr %t1366, i64 8
  store i64 %t1284, ptr %t1367
    %t1373 = inttoptr i64 %ac2 to ptr
    %t1374 = getelementptr i8, ptr %t1373, i64 6144
    %t1375 = load i64, ptr %t1374
  %t1376 = add i64 0, 1
  %t1368 = add nsw i64 %t1375, %t1376
  %t1378 = inttoptr i64 %ac2 to ptr
  %t1379 = getelementptr i8, ptr %t1378, i64 6144
  store i64 %t1368, ptr %t1379
  %t1382 = add i64 0, 1
  %t1383 = icmp sge i64 %t1289, %t1382
  %t1380 = zext i1 %t1383 to i8
  %t1385 = trunc i8 %t1380 to i1
  br i1 %t1385, label %guard.then1384, label %guard.end1384
  guard.then1384:
  %t1389 = inttoptr i64 %ac0 to ptr
  %t1390 = getelementptr i8, ptr %t1389, i64 0
  %t1393 = add i64 0, 1
  %t1391 = sub nsw i64 %t6, %t1393
  %t1394 = getelementptr [1024 x i64], ptr %t1390, i64 0, i64 %t1391
  %t1395 = load i64, ptr %t1394
  %t1398 = inttoptr i64 %ac1 to ptr
  %t1399 = getelementptr i8, ptr %t1398, i64 0
    %t1405 = inttoptr i64 %ac1 to ptr
    %t1406 = getelementptr i8, ptr %t1405, i64 32768
    %t1407 = load i64, ptr %t1406
  %t1408 = add i64 0, 0
  %t1400 = add nsw i64 %t1407, %t1408
  %t1409 = getelementptr [4096 x i64], ptr %t1399, i64 0, i64 %t1400
  store i64 %t1395, ptr %t1409
  br label %guard.end1384
  guard.end1384:
  %t1413 = add i64 0, 2
  %t1414 = icmp sge i64 %t1289, %t1413
  %t1411 = zext i1 %t1414 to i8
  %t1416 = trunc i8 %t1411 to i1
  br i1 %t1416, label %guard.then1415, label %guard.end1415
  guard.then1415:
  %t1420 = inttoptr i64 %ac0 to ptr
  %t1421 = getelementptr i8, ptr %t1420, i64 0
  %t1424 = add i64 0, 2
  %t1422 = sub nsw i64 %t6, %t1424
  %t1425 = getelementptr [1024 x i64], ptr %t1421, i64 0, i64 %t1422
  %t1426 = load i64, ptr %t1425
  %t1429 = inttoptr i64 %ac1 to ptr
  %t1430 = getelementptr i8, ptr %t1429, i64 0
    %t1436 = inttoptr i64 %ac1 to ptr
    %t1437 = getelementptr i8, ptr %t1436, i64 32768
    %t1438 = load i64, ptr %t1437
  %t1439 = add i64 0, 1
  %t1431 = add nsw i64 %t1438, %t1439
  %t1440 = getelementptr [4096 x i64], ptr %t1430, i64 0, i64 %t1431
  store i64 %t1426, ptr %t1440
  br label %guard.end1415
  guard.end1415:
  %t1444 = add i64 0, 3
  %t1445 = icmp sge i64 %t1289, %t1444
  %t1442 = zext i1 %t1445 to i8
  %t1447 = trunc i8 %t1442 to i1
  br i1 %t1447, label %guard.then1446, label %guard.end1446
  guard.then1446:
  %t1451 = inttoptr i64 %ac0 to ptr
  %t1452 = getelementptr i8, ptr %t1451, i64 0
  %t1455 = add i64 0, 3
  %t1453 = sub nsw i64 %t6, %t1455
  %t1456 = getelementptr [1024 x i64], ptr %t1452, i64 0, i64 %t1453
  %t1457 = load i64, ptr %t1456
  %t1460 = inttoptr i64 %ac1 to ptr
  %t1461 = getelementptr i8, ptr %t1460, i64 0
    %t1467 = inttoptr i64 %ac1 to ptr
    %t1468 = getelementptr i8, ptr %t1467, i64 32768
    %t1469 = load i64, ptr %t1468
  %t1470 = add i64 0, 2
  %t1462 = add nsw i64 %t1469, %t1470
  %t1471 = getelementptr [4096 x i64], ptr %t1461, i64 0, i64 %t1462
  store i64 %t1457, ptr %t1471
  br label %guard.end1446
  guard.end1446:
  %t1475 = add i64 0, 4
  %t1476 = icmp sge i64 %t1289, %t1475
  %t1473 = zext i1 %t1476 to i8
  %t1478 = trunc i8 %t1473 to i1
  br i1 %t1478, label %guard.then1477, label %guard.end1477
  guard.then1477:
  %t1482 = inttoptr i64 %ac0 to ptr
  %t1483 = getelementptr i8, ptr %t1482, i64 0
  %t1486 = add i64 0, 4
  %t1484 = sub nsw i64 %t6, %t1486
  %t1487 = getelementptr [1024 x i64], ptr %t1483, i64 0, i64 %t1484
  %t1488 = load i64, ptr %t1487
  %t1491 = inttoptr i64 %ac1 to ptr
  %t1492 = getelementptr i8, ptr %t1491, i64 0
    %t1498 = inttoptr i64 %ac1 to ptr
    %t1499 = getelementptr i8, ptr %t1498, i64 32768
    %t1500 = load i64, ptr %t1499
  %t1501 = add i64 0, 3
  %t1493 = add nsw i64 %t1500, %t1501
  %t1502 = getelementptr [4096 x i64], ptr %t1492, i64 0, i64 %t1493
  store i64 %t1488, ptr %t1502
  br label %guard.end1477
  guard.end1477:
    %t1509 = inttoptr i64 %ac1 to ptr
    %t1510 = getelementptr i8, ptr %t1509, i64 32768
    %t1511 = load i64, ptr %t1510
  %t1504 = add nsw i64 %t1511, %t1284
  %t1514 = inttoptr i64 %ac1 to ptr
  %t1515 = getelementptr i8, ptr %t1514, i64 32768
  store i64 %t1504, ptr %t1515
  %t1516 = sub nsw i64 %t6, %t1289
  %t1520 = inttoptr i64 %ac0 to ptr
  %t1521 = getelementptr i8, ptr %t1520, i64 8192
  store i64 %t1516, ptr %t1521
  ret i64 %t1279
  guard.end1304:
  %t1527 = add i64 0, 3
  %t1525 = add nsw i64 %arg8, %t1527
  ret i64 %t1525
  guard.end1277:
  %t1532 = add i64 0, 3
  %t1530 = add nsw i64 %arg8, %t1532
  ret i64 %t1530
.smt_body_8_30:
  %t1538 = add i64 0, 1
  %t1536 = add nsw i64 %arg8, %t1538
  %t1539 = inttoptr i64 %ac3 to ptr
  %t1534 = call i64 @read_i32(ptr %state, ptr %t1539, i64 %t1536)
  %t1542 = add i64 0, 1024
  %t1543 = icmp slt i64 %t6, %t1542
  %t1540 = zext i1 %t1543 to i8
  %t1545 = trunc i8 %t1540 to i1
  br i1 %t1545, label %guard.then1544, label %guard.end1544
  guard.then1544:
  %t1549 = inttoptr i64 %ac0 to ptr
  %t1550 = getelementptr i8, ptr %t1549, i64 0
  %t1552 = getelementptr [1024 x i64], ptr %t1550, i64 0, i64 %t6
  store i64 %t1534, ptr %t1552
  %t1555 = add i64 0, 1
  %t1553 = add nsw i64 %t6, %t1555
  %t1557 = inttoptr i64 %ac0 to ptr
  %t1558 = getelementptr i8, ptr %t1557, i64 8192
  store i64 %t1553, ptr %t1558
  br label %guard.end1544
  guard.end1544:
  %t1562 = add i64 0, 5
  %t1560 = add nsw i64 %arg8, %t1562
  ret i64 %t1560
.smt_body_8_31:
  %t1568 = add i64 0, 1
  %t1566 = add nsw i64 %arg8, %t1568
  %t1569 = inttoptr i64 %ac3 to ptr
  %t1564 = call i64 @read_u32(ptr %state, ptr %t1569, i64 %t1566)
   %t1570 = call i64 @briev_host_arity_of(i64 %t1564)
  %t1574 = add i64 0, 1
  %t1575 = icmp eq i64 %t1570, %t1574
  %t1572 = zext i1 %t1575 to i8
  %t1577 = trunc i8 %t1572 to i1
  br i1 %t1577, label %guard.then1576, label %guard.end1576
  guard.then1576:
  %t1581 = inttoptr i64 %ac0 to ptr
  %t1582 = getelementptr i8, ptr %t1581, i64 0
    %t1588 = inttoptr i64 %ac0 to ptr
    %t1589 = getelementptr i8, ptr %t1588, i64 8192
    %t1590 = load i64, ptr %t1589
  %t1591 = add i64 0, 1
  %t1583 = sub nsw i64 %t1590, %t1591
  %t1592 = getelementptr [1024 x i64], ptr %t1582, i64 0, i64 %t1583
  %t1593 = load i64, ptr %t1592
    %t1599 = inttoptr i64 %ac0 to ptr
    %t1600 = getelementptr i8, ptr %t1599, i64 8192
    %t1601 = load i64, ptr %t1600
  %t1602 = add i64 0, 1
  %t1594 = sub nsw i64 %t1601, %t1602
  %t1604 = inttoptr i64 %ac0 to ptr
  %t1605 = getelementptr i8, ptr %t1604, i64 8192
  store i64 %t1594, ptr %t1605
  %t1606 = call i64 @host_dispatch1(ptr %state, i64 %t1564, i64 %t1593)
  br label %guard.end1576
  guard.end1576:
  %t1612 = add i64 0, 5
  %t1610 = add nsw i64 %arg8, %t1612
  ret i64 %t1610
.smt_body_8_32:
  %t1618 = add i64 0, 1
  %t1616 = add nsw i64 %arg8, %t1618
  %t1619 = inttoptr i64 %ac3 to ptr
  %t1614 = call i64 @read_i64(ptr %state, ptr %t1619, i64 %t1616)
  %t1622 = add i64 0, 1024
  %t1623 = icmp slt i64 %t6, %t1622
  %t1620 = zext i1 %t1623 to i8
  %t1625 = trunc i8 %t1620 to i1
  br i1 %t1625, label %guard.then1624, label %guard.end1624
  guard.then1624:
  %t1629 = inttoptr i64 %ac0 to ptr
  %t1630 = getelementptr i8, ptr %t1629, i64 0
  %t1632 = getelementptr [1024 x i64], ptr %t1630, i64 0, i64 %t6
  store i64 %t1614, ptr %t1632
  %t1635 = add i64 0, 1
  %t1633 = add nsw i64 %t6, %t1635
  %t1637 = inttoptr i64 %ac0 to ptr
  %t1638 = getelementptr i8, ptr %t1637, i64 8192
  store i64 %t1633, ptr %t1638
  br label %guard.end1624
  guard.end1624:
  %t1642 = add i64 0, 9
  %t1640 = add nsw i64 %arg8, %t1642
  ret i64 %t1640
.smt_body_8_33:
  %t1646 = add i64 0, 1
  %t1647 = icmp sge i64 %t6, %t1646
  %t1644 = zext i1 %t1647 to i8
  %t1649 = trunc i8 %t1644 to i1
  br i1 %t1649, label %guard.then1648, label %guard.end1648
  guard.then1648:
  %t1654 = inttoptr i64 %ac0 to ptr
  %t1655 = getelementptr i8, ptr %t1654, i64 0
  %t1658 = add i64 0, 1
  %t1656 = sub nsw i64 %t6, %t1658
  %t1659 = getelementptr [1024 x i64], ptr %t1655, i64 0, i64 %t1656
  %t1660 = load i64, ptr %t1659
  %t1650 = xor i64 %t1660, -1
  %t1663 = inttoptr i64 %ac0 to ptr
  %t1664 = getelementptr i8, ptr %t1663, i64 0
  %t1667 = add i64 0, 1
  %t1665 = sub nsw i64 %t6, %t1667
  %t1668 = getelementptr [1024 x i64], ptr %t1664, i64 0, i64 %t1665
  store i64 %t1650, ptr %t1668
  br label %guard.end1648
  guard.end1648:
  %t1672 = add i64 0, 1
  %t1670 = add nsw i64 %arg8, %t1672
  ret i64 %t1670
.smt_body_8_34:
  %t1675 = add i64 0, 1
  %t1674 = sub i64 0, %t1675
  ret i64 %t1674
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
