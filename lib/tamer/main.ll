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
  %t41 = add i64 0, 18
  %t42 = icmp eq i64 %arg7, %t41
  br i1 %t42, label %.smt_body_8_16, label %.smt_next_8_16
.smt_next_8_16:
  %t43 = add i64 0, 19
  %t44 = icmp eq i64 %arg7, %t43
  br i1 %t44, label %.smt_body_8_17, label %.smt_next_8_17
.smt_next_8_17:
  %t45 = add i64 0, 20
  %t46 = icmp eq i64 %arg7, %t45
  br i1 %t46, label %.smt_body_8_18, label %.smt_next_8_18
.smt_next_8_18:
  %t47 = add i64 0, 21
  %t48 = icmp eq i64 %arg7, %t47
  br i1 %t48, label %.smt_body_8_19, label %.smt_next_8_19
.smt_next_8_19:
  %t49 = add i64 0, 22
  %t50 = icmp eq i64 %arg7, %t49
  br i1 %t50, label %.smt_body_8_20, label %.smt_next_8_20
.smt_next_8_20:
  %t51 = add i64 0, 23
  %t52 = icmp eq i64 %arg7, %t51
  br i1 %t52, label %.smt_body_8_21, label %.smt_next_8_21
.smt_next_8_21:
  %t53 = add i64 0, 24
  %t54 = icmp eq i64 %arg7, %t53
  br i1 %t54, label %.smt_body_8_22, label %.smt_next_8_22
.smt_next_8_22:
  %t55 = add i64 0, 25
  %t56 = icmp eq i64 %arg7, %t55
  br i1 %t56, label %.smt_body_8_23, label %.smt_next_8_23
.smt_next_8_23:
  %t57 = add i64 0, 27
  %t58 = icmp eq i64 %arg7, %t57
  br i1 %t58, label %.smt_body_8_24, label %.smt_next_8_24
.smt_next_8_24:
  %t59 = add i64 0, 48
  %t60 = icmp eq i64 %arg7, %t59
  br i1 %t60, label %.smt_body_8_25, label %.smt_next_8_25
.smt_next_8_25:
  %t61 = add i64 0, 49
  %t62 = icmp eq i64 %arg7, %t61
  br i1 %t62, label %.smt_body_8_26, label %.smt_next_8_26
.smt_next_8_26:
  %t63 = add i64 0, 50
  %t64 = icmp eq i64 %arg7, %t63
  br i1 %t64, label %.smt_body_8_27, label %.smt_next_8_27
.smt_next_8_27:
  %t65 = add i64 0, 51
  %t66 = icmp eq i64 %arg7, %t65
  br i1 %t66, label %.smt_body_8_28, label %.smt_next_8_28
.smt_next_8_28:
  %t67 = add i64 0, 52
  %t68 = icmp eq i64 %arg7, %t67
  br i1 %t68, label %.smt_body_8_29, label %.smt_next_8_29
.smt_next_8_29:
  %t69 = add i64 0, 80
  %t70 = icmp eq i64 %arg7, %t69
  br i1 %t70, label %.smt_body_8_30, label %.smt_next_8_30
.smt_next_8_30:
  %t71 = add i64 0, 81
  %t72 = icmp eq i64 %arg7, %t71
  br i1 %t72, label %.smt_body_8_31, label %.smt_next_8_31
.smt_next_8_31:
  %t73 = add i64 0, 82
  %t74 = icmp eq i64 %arg7, %t73
  br i1 %t74, label %.smt_body_8_32, label %.smt_next_8_32
.smt_next_8_32:
  %t75 = add i64 0, 83
  %t76 = icmp eq i64 %arg7, %t75
  br i1 %t76, label %.smt_body_8_33, label %.smt_next_8_33
.smt_next_8_33:
  %t77 = add i64 0, 84
  %t78 = icmp eq i64 %arg7, %t77
  br i1 %t78, label %.smt_body_8_34, label %.smt_next_8_34
.smt_next_8_34:
  %t79 = add i64 0, 112
  %t80 = icmp eq i64 %arg7, %t79
  br i1 %t80, label %.smt_body_8_35, label %.smt_next_8_35
.smt_next_8_35:
  %t81 = add i64 0, 113
  %t82 = icmp eq i64 %arg7, %t81
  br i1 %t82, label %.smt_body_8_36, label %.smt_next_8_36
.smt_next_8_36:
  %t83 = add i64 0, 144
  %t84 = icmp eq i64 %arg7, %t83
  br i1 %t84, label %.smt_body_8_37, label %.smt_next_8_37
.smt_next_8_37:
  %t85 = add i64 0, 28
  %t86 = icmp eq i64 %arg7, %t85
  br i1 %t86, label %.smt_body_8_38, label %.smt_next_8_38
.smt_next_8_38:
  %t87 = icmp eq i64 0, 0
  br i1 %t87, label %.smt_body_8_39, label %.smt_next_8_39
.smt_next_8_39:
  br label %.smt_end_8
.smt_body_8_0:
  %t90 = add i64 0, 1
  %t88 = add nsw i64 %arg8, %t90
  ret i64 %t88
.smt_body_8_1:
  %t94 = add i64 0, 0
  %t95 = icmp sgt i64 %t6, %t94
  %t92 = zext i1 %t95 to i8
  %t97 = trunc i8 %t92 to i1
  br i1 %t97, label %guard.then96, label %guard.end96
  guard.then96:
  %t100 = add i64 0, 1
  %t98 = sub nsw i64 %t6, %t100
  %t102 = inttoptr i64 %ac0 to ptr
  %t103 = getelementptr i8, ptr %t102, i64 8192
  store i64 %t98, ptr %t103
  br label %guard.end96
  guard.end96:
  %t107 = add i64 0, 1
  %t105 = add nsw i64 %arg8, %t107
  ret i64 %t105
.smt_body_8_2:
  %t111 = add i64 0, 0
  %t112 = icmp sgt i64 %t6, %t111
  %t109 = zext i1 %t112 to i8
  %t114 = trunc i8 %t109 to i1
  br i1 %t114, label %guard.then113, label %guard.end113
  guard.then113:
  %t118 = inttoptr i64 %ac0 to ptr
  %t119 = getelementptr i8, ptr %t118, i64 0
  %t122 = add i64 0, 1
  %t120 = sub nsw i64 %t6, %t122
  %t123 = getelementptr [1024 x i64], ptr %t119, i64 0, i64 %t120
  %t124 = load i64, ptr %t123
  %t127 = inttoptr i64 %ac0 to ptr
  %t128 = getelementptr i8, ptr %t127, i64 0
  %t130 = getelementptr [1024 x i64], ptr %t128, i64 0, i64 %t6
  store i64 %t124, ptr %t130
  %t133 = add i64 0, 1
  %t131 = add nsw i64 %t6, %t133
  %t135 = inttoptr i64 %ac0 to ptr
  %t136 = getelementptr i8, ptr %t135, i64 8192
  store i64 %t131, ptr %t136
  br label %guard.end113
  guard.end113:
  %t140 = add i64 0, 1
  %t138 = add nsw i64 %arg8, %t140
  ret i64 %t138
.smt_body_8_3:
  %t144 = add i64 0, 2
  %t145 = icmp sge i64 %t6, %t144
  %t142 = zext i1 %t145 to i8
  %t147 = trunc i8 %t142 to i1
  br i1 %t147, label %guard.then146, label %guard.end146
  guard.then146:
  %t151 = inttoptr i64 %ac0 to ptr
  %t152 = getelementptr i8, ptr %t151, i64 0
  %t155 = add i64 0, 1
  %t153 = sub nsw i64 %t6, %t155
  %t156 = getelementptr [1024 x i64], ptr %t152, i64 0, i64 %t153
  %t157 = load i64, ptr %t156
  %t161 = inttoptr i64 %ac0 to ptr
  %t162 = getelementptr i8, ptr %t161, i64 0
  %t165 = add i64 0, 2
  %t163 = sub nsw i64 %t6, %t165
  %t166 = getelementptr [1024 x i64], ptr %t162, i64 0, i64 %t163
  %t167 = load i64, ptr %t166
  %t171 = inttoptr i64 %ac0 to ptr
  %t172 = getelementptr i8, ptr %t171, i64 0
  %t175 = add i64 0, 2
  %t173 = sub nsw i64 %t6, %t175
  %t176 = getelementptr [1024 x i64], ptr %t172, i64 0, i64 %t173
  store i64 %t157, ptr %t176
  %t180 = inttoptr i64 %ac0 to ptr
  %t181 = getelementptr i8, ptr %t180, i64 0
  %t184 = add i64 0, 1
  %t182 = sub nsw i64 %t6, %t184
  %t185 = getelementptr [1024 x i64], ptr %t181, i64 0, i64 %t182
  store i64 %t167, ptr %t185
  br label %guard.end146
  guard.end146:
  %t189 = add i64 0, 1
  %t187 = add nsw i64 %arg8, %t189
  ret i64 %t187
.smt_body_8_4:
  %t193 = add i64 0, 2
  %t194 = icmp sge i64 %t6, %t193
  %t191 = zext i1 %t194 to i8
  %t196 = trunc i8 %t191 to i1
  br i1 %t196, label %guard.then195, label %guard.end195
  guard.then195:
  %t201 = inttoptr i64 %ac0 to ptr
  %t202 = getelementptr i8, ptr %t201, i64 0
  %t205 = add i64 0, 2
  %t203 = sub nsw i64 %t6, %t205
  %t206 = getelementptr [1024 x i64], ptr %t202, i64 0, i64 %t203
  %t207 = load i64, ptr %t206
  %t211 = inttoptr i64 %ac0 to ptr
  %t212 = getelementptr i8, ptr %t211, i64 0
  %t215 = add i64 0, 1
  %t213 = sub nsw i64 %t6, %t215
  %t216 = getelementptr [1024 x i64], ptr %t212, i64 0, i64 %t213
  %t217 = load i64, ptr %t216
  %t197 = add nsw i64 %t207, %t217
  %t220 = inttoptr i64 %ac0 to ptr
  %t221 = getelementptr i8, ptr %t220, i64 0
  %t224 = add i64 0, 2
  %t222 = sub nsw i64 %t6, %t224
  %t225 = getelementptr [1024 x i64], ptr %t221, i64 0, i64 %t222
  store i64 %t197, ptr %t225
  %t228 = add i64 0, 1
  %t226 = sub nsw i64 %t6, %t228
  %t230 = inttoptr i64 %ac0 to ptr
  %t231 = getelementptr i8, ptr %t230, i64 8192
  store i64 %t226, ptr %t231
  br label %guard.end195
  guard.end195:
  %t235 = add i64 0, 1
  %t233 = add nsw i64 %arg8, %t235
  ret i64 %t233
.smt_body_8_5:
  %t239 = add i64 0, 2
  %t240 = icmp sge i64 %t6, %t239
  %t237 = zext i1 %t240 to i8
  %t242 = trunc i8 %t237 to i1
  br i1 %t242, label %guard.then241, label %guard.end241
  guard.then241:
  %t247 = inttoptr i64 %ac0 to ptr
  %t248 = getelementptr i8, ptr %t247, i64 0
  %t251 = add i64 0, 2
  %t249 = sub nsw i64 %t6, %t251
  %t252 = getelementptr [1024 x i64], ptr %t248, i64 0, i64 %t249
  %t253 = load i64, ptr %t252
  %t257 = inttoptr i64 %ac0 to ptr
  %t258 = getelementptr i8, ptr %t257, i64 0
  %t261 = add i64 0, 1
  %t259 = sub nsw i64 %t6, %t261
  %t262 = getelementptr [1024 x i64], ptr %t258, i64 0, i64 %t259
  %t263 = load i64, ptr %t262
  %t243 = sub nsw i64 %t253, %t263
  %t266 = inttoptr i64 %ac0 to ptr
  %t267 = getelementptr i8, ptr %t266, i64 0
  %t270 = add i64 0, 2
  %t268 = sub nsw i64 %t6, %t270
  %t271 = getelementptr [1024 x i64], ptr %t267, i64 0, i64 %t268
  store i64 %t243, ptr %t271
  %t274 = add i64 0, 1
  %t272 = sub nsw i64 %t6, %t274
  %t276 = inttoptr i64 %ac0 to ptr
  %t277 = getelementptr i8, ptr %t276, i64 8192
  store i64 %t272, ptr %t277
  br label %guard.end241
  guard.end241:
  %t281 = add i64 0, 1
  %t279 = add nsw i64 %arg8, %t281
  ret i64 %t279
.smt_body_8_6:
  %t285 = add i64 0, 2
  %t286 = icmp sge i64 %t6, %t285
  %t283 = zext i1 %t286 to i8
  %t288 = trunc i8 %t283 to i1
  br i1 %t288, label %guard.then287, label %guard.end287
  guard.then287:
  %t293 = inttoptr i64 %ac0 to ptr
  %t294 = getelementptr i8, ptr %t293, i64 0
  %t297 = add i64 0, 2
  %t295 = sub nsw i64 %t6, %t297
  %t298 = getelementptr [1024 x i64], ptr %t294, i64 0, i64 %t295
  %t299 = load i64, ptr %t298
  %t303 = inttoptr i64 %ac0 to ptr
  %t304 = getelementptr i8, ptr %t303, i64 0
  %t307 = add i64 0, 1
  %t305 = sub nsw i64 %t6, %t307
  %t308 = getelementptr [1024 x i64], ptr %t304, i64 0, i64 %t305
  %t309 = load i64, ptr %t308
  %t289 = mul nsw i64 %t299, %t309
  %t312 = inttoptr i64 %ac0 to ptr
  %t313 = getelementptr i8, ptr %t312, i64 0
  %t316 = add i64 0, 2
  %t314 = sub nsw i64 %t6, %t316
  %t317 = getelementptr [1024 x i64], ptr %t313, i64 0, i64 %t314
  store i64 %t289, ptr %t317
  %t320 = add i64 0, 1
  %t318 = sub nsw i64 %t6, %t320
  %t322 = inttoptr i64 %ac0 to ptr
  %t323 = getelementptr i8, ptr %t322, i64 8192
  store i64 %t318, ptr %t323
  br label %guard.end287
  guard.end287:
  %t327 = add i64 0, 1
  %t325 = add nsw i64 %arg8, %t327
  ret i64 %t325
.smt_body_8_7:
  %t331 = add i64 0, 2
  %t332 = icmp sge i64 %t6, %t331
  %t329 = zext i1 %t332 to i8
  %t334 = trunc i8 %t329 to i1
  br i1 %t334, label %guard.then333, label %guard.end333
  guard.then333:
  %t338 = inttoptr i64 %ac0 to ptr
  %t339 = getelementptr i8, ptr %t338, i64 0
  %t342 = add i64 0, 1
  %t340 = sub nsw i64 %t6, %t342
  %t343 = getelementptr [1024 x i64], ptr %t339, i64 0, i64 %t340
  %t344 = load i64, ptr %t343
  %t348 = inttoptr i64 %ac0 to ptr
  %t349 = getelementptr i8, ptr %t348, i64 0
  %t352 = add i64 0, 2
  %t350 = sub nsw i64 %t6, %t352
  %t353 = getelementptr [1024 x i64], ptr %t349, i64 0, i64 %t350
  %t354 = load i64, ptr %t353
  %t357 = add i64 0, 0
  %t358 = icmp ne i64 %t344, %t357
  %t355 = zext i1 %t358 to i8
  %t360 = trunc i8 %t355 to i1
  br i1 %t360, label %guard.then359, label %guard.end359
  guard.then359:
  %t361 = sdiv i64 %t354, %t344
  %t366 = inttoptr i64 %ac0 to ptr
  %t367 = getelementptr i8, ptr %t366, i64 0
  %t370 = add i64 0, 2
  %t368 = sub nsw i64 %t6, %t370
  %t371 = getelementptr [1024 x i64], ptr %t367, i64 0, i64 %t368
  store i64 %t361, ptr %t371
  %t374 = add i64 0, 1
  %t372 = sub nsw i64 %t6, %t374
  %t376 = inttoptr i64 %ac0 to ptr
  %t377 = getelementptr i8, ptr %t376, i64 8192
  store i64 %t372, ptr %t377
  br label %guard.end359
  guard.end359:
  br label %guard.end333
  guard.end333:
  %t382 = add i64 0, 1
  %t380 = add nsw i64 %arg8, %t382
  ret i64 %t380
.smt_body_8_8:
  %t386 = add i64 0, 2
  %t387 = icmp sge i64 %t6, %t386
  %t384 = zext i1 %t387 to i8
  %t389 = trunc i8 %t384 to i1
  br i1 %t389, label %guard.then388, label %guard.end388
  guard.then388:
  %t393 = inttoptr i64 %ac0 to ptr
  %t394 = getelementptr i8, ptr %t393, i64 0
  %t397 = add i64 0, 1
  %t395 = sub nsw i64 %t6, %t397
  %t398 = getelementptr [1024 x i64], ptr %t394, i64 0, i64 %t395
  %t399 = load i64, ptr %t398
  %t403 = inttoptr i64 %ac0 to ptr
  %t404 = getelementptr i8, ptr %t403, i64 0
  %t407 = add i64 0, 2
  %t405 = sub nsw i64 %t6, %t407
  %t408 = getelementptr [1024 x i64], ptr %t404, i64 0, i64 %t405
  %t409 = load i64, ptr %t408
  %t412 = add i64 0, 0
  %t413 = icmp ne i64 %t399, %t412
  %t410 = zext i1 %t413 to i8
  %t415 = trunc i8 %t410 to i1
  br i1 %t415, label %guard.then414, label %guard.end414
  guard.then414:
  %t416 = srem i64 %t409, %t399
  %t421 = inttoptr i64 %ac0 to ptr
  %t422 = getelementptr i8, ptr %t421, i64 0
  %t425 = add i64 0, 2
  %t423 = sub nsw i64 %t6, %t425
  %t426 = getelementptr [1024 x i64], ptr %t422, i64 0, i64 %t423
  store i64 %t416, ptr %t426
  %t429 = add i64 0, 1
  %t427 = sub nsw i64 %t6, %t429
  %t431 = inttoptr i64 %ac0 to ptr
  %t432 = getelementptr i8, ptr %t431, i64 8192
  store i64 %t427, ptr %t432
  br label %guard.end414
  guard.end414:
  br label %guard.end388
  guard.end388:
  %t437 = add i64 0, 1
  %t435 = add nsw i64 %arg8, %t437
  ret i64 %t435
.smt_body_8_9:
  %t441 = add i64 0, 2
  %t442 = icmp sge i64 %t6, %t441
  %t439 = zext i1 %t442 to i8
  %t444 = trunc i8 %t439 to i1
  br i1 %t444, label %guard.then443, label %guard.end443
  guard.then443:
  %t449 = inttoptr i64 %ac0 to ptr
  %t450 = getelementptr i8, ptr %t449, i64 0
  %t453 = add i64 0, 2
  %t451 = sub nsw i64 %t6, %t453
  %t454 = getelementptr [1024 x i64], ptr %t450, i64 0, i64 %t451
  %t455 = load i64, ptr %t454
  %t459 = inttoptr i64 %ac0 to ptr
  %t460 = getelementptr i8, ptr %t459, i64 0
  %t463 = add i64 0, 1
  %t461 = sub nsw i64 %t6, %t463
  %t464 = getelementptr [1024 x i64], ptr %t460, i64 0, i64 %t461
  %t465 = load i64, ptr %t464
  %t445 = and i64 %t455, %t465
  %t468 = inttoptr i64 %ac0 to ptr
  %t469 = getelementptr i8, ptr %t468, i64 0
  %t472 = add i64 0, 2
  %t470 = sub nsw i64 %t6, %t472
  %t473 = getelementptr [1024 x i64], ptr %t469, i64 0, i64 %t470
  store i64 %t445, ptr %t473
  %t476 = add i64 0, 1
  %t474 = sub nsw i64 %t6, %t476
  %t478 = inttoptr i64 %ac0 to ptr
  %t479 = getelementptr i8, ptr %t478, i64 8192
  store i64 %t474, ptr %t479
  br label %guard.end443
  guard.end443:
  %t483 = add i64 0, 1
  %t481 = add nsw i64 %arg8, %t483
  ret i64 %t481
.smt_body_8_10:
  %t487 = add i64 0, 2
  %t488 = icmp sge i64 %t6, %t487
  %t485 = zext i1 %t488 to i8
  %t490 = trunc i8 %t485 to i1
  br i1 %t490, label %guard.then489, label %guard.end489
  guard.then489:
  %t495 = inttoptr i64 %ac0 to ptr
  %t496 = getelementptr i8, ptr %t495, i64 0
  %t499 = add i64 0, 2
  %t497 = sub nsw i64 %t6, %t499
  %t500 = getelementptr [1024 x i64], ptr %t496, i64 0, i64 %t497
  %t501 = load i64, ptr %t500
  %t505 = inttoptr i64 %ac0 to ptr
  %t506 = getelementptr i8, ptr %t505, i64 0
  %t509 = add i64 0, 1
  %t507 = sub nsw i64 %t6, %t509
  %t510 = getelementptr [1024 x i64], ptr %t506, i64 0, i64 %t507
  %t511 = load i64, ptr %t510
  %t491 = or i64 %t501, %t511
  %t514 = inttoptr i64 %ac0 to ptr
  %t515 = getelementptr i8, ptr %t514, i64 0
  %t518 = add i64 0, 2
  %t516 = sub nsw i64 %t6, %t518
  %t519 = getelementptr [1024 x i64], ptr %t515, i64 0, i64 %t516
  store i64 %t491, ptr %t519
  %t522 = add i64 0, 1
  %t520 = sub nsw i64 %t6, %t522
  %t524 = inttoptr i64 %ac0 to ptr
  %t525 = getelementptr i8, ptr %t524, i64 8192
  store i64 %t520, ptr %t525
  br label %guard.end489
  guard.end489:
  %t529 = add i64 0, 1
  %t527 = add nsw i64 %arg8, %t529
  ret i64 %t527
.smt_body_8_11:
  %t533 = add i64 0, 2
  %t534 = icmp sge i64 %t6, %t533
  %t531 = zext i1 %t534 to i8
  %t536 = trunc i8 %t531 to i1
  br i1 %t536, label %guard.then535, label %guard.end535
  guard.then535:
  %t541 = inttoptr i64 %ac0 to ptr
  %t542 = getelementptr i8, ptr %t541, i64 0
  %t545 = add i64 0, 2
  %t543 = sub nsw i64 %t6, %t545
  %t546 = getelementptr [1024 x i64], ptr %t542, i64 0, i64 %t543
  %t547 = load i64, ptr %t546
  %t551 = inttoptr i64 %ac0 to ptr
  %t552 = getelementptr i8, ptr %t551, i64 0
  %t555 = add i64 0, 1
  %t553 = sub nsw i64 %t6, %t555
  %t556 = getelementptr [1024 x i64], ptr %t552, i64 0, i64 %t553
  %t557 = load i64, ptr %t556
  %t537 = xor i64 %t547, %t557
  %t560 = inttoptr i64 %ac0 to ptr
  %t561 = getelementptr i8, ptr %t560, i64 0
  %t564 = add i64 0, 2
  %t562 = sub nsw i64 %t6, %t564
  %t565 = getelementptr [1024 x i64], ptr %t561, i64 0, i64 %t562
  store i64 %t537, ptr %t565
  %t568 = add i64 0, 1
  %t566 = sub nsw i64 %t6, %t568
  %t570 = inttoptr i64 %ac0 to ptr
  %t571 = getelementptr i8, ptr %t570, i64 8192
  store i64 %t566, ptr %t571
  br label %guard.end535
  guard.end535:
  %t575 = add i64 0, 1
  %t573 = add nsw i64 %arg8, %t575
  ret i64 %t573
.smt_body_8_12:
  %t579 = add i64 0, 1
  %t580 = icmp sge i64 %t6, %t579
  %t577 = zext i1 %t580 to i8
  %t582 = trunc i8 %t577 to i1
  br i1 %t582, label %guard.then581, label %guard.end581
  guard.then581:
  %t586 = inttoptr i64 %ac0 to ptr
  %t587 = getelementptr i8, ptr %t586, i64 0
  %t590 = add i64 0, 1
  %t588 = sub nsw i64 %t6, %t590
  %t591 = getelementptr [1024 x i64], ptr %t587, i64 0, i64 %t588
  %t592 = load i64, ptr %t591
  %t593 = add i64 0, 0
  %t596 = inttoptr i64 %ac0 to ptr
  %t597 = getelementptr i8, ptr %t596, i64 0
  %t600 = add i64 0, 1
  %t598 = sub nsw i64 %t6, %t600
  %t601 = getelementptr [1024 x i64], ptr %t597, i64 0, i64 %t598
  store i64 %t593, ptr %t601
  %t604 = add i64 0, 0
  %t605 = icmp eq i64 %t592, %t604
  %t602 = zext i1 %t605 to i8
  %t607 = trunc i8 %t602 to i1
  br i1 %t607, label %guard.then606, label %guard.end606
  guard.then606:
  %t608 = add i64 0, 1
  %t611 = inttoptr i64 %ac0 to ptr
  %t612 = getelementptr i8, ptr %t611, i64 0
  %t615 = add i64 0, 1
  %t613 = sub nsw i64 %t6, %t615
  %t616 = getelementptr [1024 x i64], ptr %t612, i64 0, i64 %t613
  store i64 %t608, ptr %t616
  br label %guard.end606
  guard.end606:
  br label %guard.end581
  guard.end581:
  %t621 = add i64 0, 1
  %t619 = add nsw i64 %arg8, %t621
  ret i64 %t619
.smt_body_8_13:
  %t625 = add i64 0, 2
  %t626 = icmp sge i64 %t6, %t625
  %t623 = zext i1 %t626 to i8
  %t628 = trunc i8 %t623 to i1
  br i1 %t628, label %guard.then627, label %guard.end627
  guard.then627:
  %t633 = inttoptr i64 %ac0 to ptr
  %t634 = getelementptr i8, ptr %t633, i64 0
  %t637 = add i64 0, 2
  %t635 = sub nsw i64 %t6, %t637
  %t638 = getelementptr [1024 x i64], ptr %t634, i64 0, i64 %t635
  %t639 = load i64, ptr %t638
  %t644 = inttoptr i64 %ac0 to ptr
  %t645 = getelementptr i8, ptr %t644, i64 0
  %t648 = add i64 0, 1
  %t646 = sub nsw i64 %t6, %t648
  %t649 = getelementptr [1024 x i64], ptr %t645, i64 0, i64 %t646
  %t650 = load i64, ptr %t649
  %t651 = add i64 0, 63
  %t640 = and i64 %t650, %t651
  %t629 = shl i64 %t639, %t640
  %t654 = inttoptr i64 %ac0 to ptr
  %t655 = getelementptr i8, ptr %t654, i64 0
  %t658 = add i64 0, 2
  %t656 = sub nsw i64 %t6, %t658
  %t659 = getelementptr [1024 x i64], ptr %t655, i64 0, i64 %t656
  store i64 %t629, ptr %t659
  %t662 = add i64 0, 1
  %t660 = sub nsw i64 %t6, %t662
  %t664 = inttoptr i64 %ac0 to ptr
  %t665 = getelementptr i8, ptr %t664, i64 8192
  store i64 %t660, ptr %t665
  br label %guard.end627
  guard.end627:
  %t669 = add i64 0, 1
  %t667 = add nsw i64 %arg8, %t669
  ret i64 %t667
.smt_body_8_14:
  %t673 = add i64 0, 2
  %t674 = icmp sge i64 %t6, %t673
  %t671 = zext i1 %t674 to i8
  %t676 = trunc i8 %t671 to i1
  br i1 %t676, label %guard.then675, label %guard.end675
  guard.then675:
  %t681 = inttoptr i64 %ac0 to ptr
  %t682 = getelementptr i8, ptr %t681, i64 0
  %t685 = add i64 0, 2
  %t683 = sub nsw i64 %t6, %t685
  %t686 = getelementptr [1024 x i64], ptr %t682, i64 0, i64 %t683
  %t687 = load i64, ptr %t686
  %t692 = inttoptr i64 %ac0 to ptr
  %t693 = getelementptr i8, ptr %t692, i64 0
  %t696 = add i64 0, 1
  %t694 = sub nsw i64 %t6, %t696
  %t697 = getelementptr [1024 x i64], ptr %t693, i64 0, i64 %t694
  %t698 = load i64, ptr %t697
  %t699 = add i64 0, 63
  %t688 = and i64 %t698, %t699
  %t677 = ashr i64 %t687, %t688
  %t702 = inttoptr i64 %ac0 to ptr
  %t703 = getelementptr i8, ptr %t702, i64 0
  %t706 = add i64 0, 2
  %t704 = sub nsw i64 %t6, %t706
  %t707 = getelementptr [1024 x i64], ptr %t703, i64 0, i64 %t704
  store i64 %t677, ptr %t707
  %t710 = add i64 0, 1
  %t708 = sub nsw i64 %t6, %t710
  %t712 = inttoptr i64 %ac0 to ptr
  %t713 = getelementptr i8, ptr %t712, i64 8192
  store i64 %t708, ptr %t713
  br label %guard.end675
  guard.end675:
  %t717 = add i64 0, 1
  %t715 = add nsw i64 %arg8, %t717
  ret i64 %t715
.smt_body_8_15:
  %t721 = add i64 0, 2
  %t722 = icmp sge i64 %t6, %t721
  %t719 = zext i1 %t722 to i8
  %t724 = trunc i8 %t719 to i1
  br i1 %t724, label %guard.then723, label %guard.end723
  guard.then723:
  %t728 = inttoptr i64 %ac0 to ptr
  %t729 = getelementptr i8, ptr %t728, i64 0
  %t732 = add i64 0, 1
  %t730 = sub nsw i64 %t6, %t732
  %t733 = getelementptr [1024 x i64], ptr %t729, i64 0, i64 %t730
  %t734 = load i64, ptr %t733
  %t738 = inttoptr i64 %ac0 to ptr
  %t739 = getelementptr i8, ptr %t738, i64 0
  %t742 = add i64 0, 2
  %t740 = sub nsw i64 %t6, %t742
  %t743 = getelementptr [1024 x i64], ptr %t739, i64 0, i64 %t740
  %t744 = load i64, ptr %t743
  %t745 = add i64 0, 0
  %t748 = inttoptr i64 %ac0 to ptr
  %t749 = getelementptr i8, ptr %t748, i64 0
  %t752 = add i64 0, 2
  %t750 = sub nsw i64 %t6, %t752
  %t753 = getelementptr [1024 x i64], ptr %t749, i64 0, i64 %t750
  store i64 %t745, ptr %t753
  %t757 = icmp eq i64 %t744, %t734
  %t754 = zext i1 %t757 to i8
  %t759 = trunc i8 %t754 to i1
  br i1 %t759, label %guard.then758, label %guard.end758
  guard.then758:
  %t760 = add i64 0, 1
  %t763 = inttoptr i64 %ac0 to ptr
  %t764 = getelementptr i8, ptr %t763, i64 0
  %t767 = add i64 0, 2
  %t765 = sub nsw i64 %t6, %t767
  %t768 = getelementptr [1024 x i64], ptr %t764, i64 0, i64 %t765
  store i64 %t760, ptr %t768
  br label %guard.end758
  guard.end758:
  %t772 = add i64 0, 1
  %t770 = sub nsw i64 %t6, %t772
  %t774 = inttoptr i64 %ac0 to ptr
  %t775 = getelementptr i8, ptr %t774, i64 8192
  store i64 %t770, ptr %t775
  br label %guard.end723
  guard.end723:
  %t779 = add i64 0, 1
  %t777 = add nsw i64 %arg8, %t779
  ret i64 %t777
.smt_body_8_16:
  %t783 = add i64 0, 2
  %t784 = icmp sge i64 %t6, %t783
  %t781 = zext i1 %t784 to i8
  %t786 = trunc i8 %t781 to i1
  br i1 %t786, label %guard.then785, label %guard.end785
  guard.then785:
  %t790 = inttoptr i64 %ac0 to ptr
  %t791 = getelementptr i8, ptr %t790, i64 0
  %t794 = add i64 0, 1
  %t792 = sub nsw i64 %t6, %t794
  %t795 = getelementptr [1024 x i64], ptr %t791, i64 0, i64 %t792
  %t796 = load i64, ptr %t795
  %t800 = inttoptr i64 %ac0 to ptr
  %t801 = getelementptr i8, ptr %t800, i64 0
  %t804 = add i64 0, 2
  %t802 = sub nsw i64 %t6, %t804
  %t805 = getelementptr [1024 x i64], ptr %t801, i64 0, i64 %t802
  %t806 = load i64, ptr %t805
  %t807 = add i64 0, 1
  %t810 = inttoptr i64 %ac0 to ptr
  %t811 = getelementptr i8, ptr %t810, i64 0
  %t814 = add i64 0, 2
  %t812 = sub nsw i64 %t6, %t814
  %t815 = getelementptr [1024 x i64], ptr %t811, i64 0, i64 %t812
  store i64 %t807, ptr %t815
  %t819 = icmp eq i64 %t806, %t796
  %t816 = zext i1 %t819 to i8
  %t821 = trunc i8 %t816 to i1
  br i1 %t821, label %guard.then820, label %guard.end820
  guard.then820:
  %t822 = add i64 0, 0
  %t825 = inttoptr i64 %ac0 to ptr
  %t826 = getelementptr i8, ptr %t825, i64 0
  %t829 = add i64 0, 2
  %t827 = sub nsw i64 %t6, %t829
  %t830 = getelementptr [1024 x i64], ptr %t826, i64 0, i64 %t827
  store i64 %t822, ptr %t830
  br label %guard.end820
  guard.end820:
  %t834 = add i64 0, 1
  %t832 = sub nsw i64 %t6, %t834
  %t836 = inttoptr i64 %ac0 to ptr
  %t837 = getelementptr i8, ptr %t836, i64 8192
  store i64 %t832, ptr %t837
  br label %guard.end785
  guard.end785:
  %t841 = add i64 0, 1
  %t839 = add nsw i64 %arg8, %t841
  ret i64 %t839
.smt_body_8_17:
  %t845 = add i64 0, 2
  %t846 = icmp sge i64 %t6, %t845
  %t843 = zext i1 %t846 to i8
  %t848 = trunc i8 %t843 to i1
  br i1 %t848, label %guard.then847, label %guard.end847
  guard.then847:
  %t852 = inttoptr i64 %ac0 to ptr
  %t853 = getelementptr i8, ptr %t852, i64 0
  %t856 = add i64 0, 1
  %t854 = sub nsw i64 %t6, %t856
  %t857 = getelementptr [1024 x i64], ptr %t853, i64 0, i64 %t854
  %t858 = load i64, ptr %t857
  %t862 = inttoptr i64 %ac0 to ptr
  %t863 = getelementptr i8, ptr %t862, i64 0
  %t866 = add i64 0, 2
  %t864 = sub nsw i64 %t6, %t866
  %t867 = getelementptr [1024 x i64], ptr %t863, i64 0, i64 %t864
  %t868 = load i64, ptr %t867
  %t869 = add i64 0, 0
  %t872 = inttoptr i64 %ac0 to ptr
  %t873 = getelementptr i8, ptr %t872, i64 0
  %t876 = add i64 0, 2
  %t874 = sub nsw i64 %t6, %t876
  %t877 = getelementptr [1024 x i64], ptr %t873, i64 0, i64 %t874
  store i64 %t869, ptr %t877
  %t881 = icmp slt i64 %t868, %t858
  %t878 = zext i1 %t881 to i8
  %t883 = trunc i8 %t878 to i1
  br i1 %t883, label %guard.then882, label %guard.end882
  guard.then882:
  %t884 = add i64 0, 1
  %t887 = inttoptr i64 %ac0 to ptr
  %t888 = getelementptr i8, ptr %t887, i64 0
  %t891 = add i64 0, 2
  %t889 = sub nsw i64 %t6, %t891
  %t892 = getelementptr [1024 x i64], ptr %t888, i64 0, i64 %t889
  store i64 %t884, ptr %t892
  br label %guard.end882
  guard.end882:
  %t896 = add i64 0, 1
  %t894 = sub nsw i64 %t6, %t896
  %t898 = inttoptr i64 %ac0 to ptr
  %t899 = getelementptr i8, ptr %t898, i64 8192
  store i64 %t894, ptr %t899
  br label %guard.end847
  guard.end847:
  %t903 = add i64 0, 1
  %t901 = add nsw i64 %arg8, %t903
  ret i64 %t901
.smt_body_8_18:
  %t907 = add i64 0, 2
  %t908 = icmp sge i64 %t6, %t907
  %t905 = zext i1 %t908 to i8
  %t910 = trunc i8 %t905 to i1
  br i1 %t910, label %guard.then909, label %guard.end909
  guard.then909:
  %t914 = inttoptr i64 %ac0 to ptr
  %t915 = getelementptr i8, ptr %t914, i64 0
  %t918 = add i64 0, 1
  %t916 = sub nsw i64 %t6, %t918
  %t919 = getelementptr [1024 x i64], ptr %t915, i64 0, i64 %t916
  %t920 = load i64, ptr %t919
  %t924 = inttoptr i64 %ac0 to ptr
  %t925 = getelementptr i8, ptr %t924, i64 0
  %t928 = add i64 0, 2
  %t926 = sub nsw i64 %t6, %t928
  %t929 = getelementptr [1024 x i64], ptr %t925, i64 0, i64 %t926
  %t930 = load i64, ptr %t929
  %t931 = add i64 0, 0
  %t934 = inttoptr i64 %ac0 to ptr
  %t935 = getelementptr i8, ptr %t934, i64 0
  %t938 = add i64 0, 2
  %t936 = sub nsw i64 %t6, %t938
  %t939 = getelementptr [1024 x i64], ptr %t935, i64 0, i64 %t936
  store i64 %t931, ptr %t939
  %t943 = icmp sle i64 %t930, %t920
  %t940 = zext i1 %t943 to i8
  %t945 = trunc i8 %t940 to i1
  br i1 %t945, label %guard.then944, label %guard.end944
  guard.then944:
  %t946 = add i64 0, 1
  %t949 = inttoptr i64 %ac0 to ptr
  %t950 = getelementptr i8, ptr %t949, i64 0
  %t953 = add i64 0, 2
  %t951 = sub nsw i64 %t6, %t953
  %t954 = getelementptr [1024 x i64], ptr %t950, i64 0, i64 %t951
  store i64 %t946, ptr %t954
  br label %guard.end944
  guard.end944:
  %t958 = add i64 0, 1
  %t956 = sub nsw i64 %t6, %t958
  %t960 = inttoptr i64 %ac0 to ptr
  %t961 = getelementptr i8, ptr %t960, i64 8192
  store i64 %t956, ptr %t961
  br label %guard.end909
  guard.end909:
  %t965 = add i64 0, 1
  %t963 = add nsw i64 %arg8, %t965
  ret i64 %t963
.smt_body_8_19:
  %t969 = add i64 0, 2
  %t970 = icmp sge i64 %t6, %t969
  %t967 = zext i1 %t970 to i8
  %t972 = trunc i8 %t967 to i1
  br i1 %t972, label %guard.then971, label %guard.end971
  guard.then971:
  %t976 = inttoptr i64 %ac0 to ptr
  %t977 = getelementptr i8, ptr %t976, i64 0
  %t980 = add i64 0, 1
  %t978 = sub nsw i64 %t6, %t980
  %t981 = getelementptr [1024 x i64], ptr %t977, i64 0, i64 %t978
  %t982 = load i64, ptr %t981
  %t986 = inttoptr i64 %ac0 to ptr
  %t987 = getelementptr i8, ptr %t986, i64 0
  %t990 = add i64 0, 2
  %t988 = sub nsw i64 %t6, %t990
  %t991 = getelementptr [1024 x i64], ptr %t987, i64 0, i64 %t988
  %t992 = load i64, ptr %t991
  %t993 = add i64 0, 0
  %t996 = inttoptr i64 %ac0 to ptr
  %t997 = getelementptr i8, ptr %t996, i64 0
  %t1000 = add i64 0, 2
  %t998 = sub nsw i64 %t6, %t1000
  %t1001 = getelementptr [1024 x i64], ptr %t997, i64 0, i64 %t998
  store i64 %t993, ptr %t1001
  %t1005 = icmp sgt i64 %t992, %t982
  %t1002 = zext i1 %t1005 to i8
  %t1007 = trunc i8 %t1002 to i1
  br i1 %t1007, label %guard.then1006, label %guard.end1006
  guard.then1006:
  %t1008 = add i64 0, 1
  %t1011 = inttoptr i64 %ac0 to ptr
  %t1012 = getelementptr i8, ptr %t1011, i64 0
  %t1015 = add i64 0, 2
  %t1013 = sub nsw i64 %t6, %t1015
  %t1016 = getelementptr [1024 x i64], ptr %t1012, i64 0, i64 %t1013
  store i64 %t1008, ptr %t1016
  br label %guard.end1006
  guard.end1006:
  %t1020 = add i64 0, 1
  %t1018 = sub nsw i64 %t6, %t1020
  %t1022 = inttoptr i64 %ac0 to ptr
  %t1023 = getelementptr i8, ptr %t1022, i64 8192
  store i64 %t1018, ptr %t1023
  br label %guard.end971
  guard.end971:
  %t1027 = add i64 0, 1
  %t1025 = add nsw i64 %arg8, %t1027
  ret i64 %t1025
.smt_body_8_20:
  %t1031 = add i64 0, 2
  %t1032 = icmp sge i64 %t6, %t1031
  %t1029 = zext i1 %t1032 to i8
  %t1034 = trunc i8 %t1029 to i1
  br i1 %t1034, label %guard.then1033, label %guard.end1033
  guard.then1033:
  %t1038 = inttoptr i64 %ac0 to ptr
  %t1039 = getelementptr i8, ptr %t1038, i64 0
  %t1042 = add i64 0, 1
  %t1040 = sub nsw i64 %t6, %t1042
  %t1043 = getelementptr [1024 x i64], ptr %t1039, i64 0, i64 %t1040
  %t1044 = load i64, ptr %t1043
  %t1048 = inttoptr i64 %ac0 to ptr
  %t1049 = getelementptr i8, ptr %t1048, i64 0
  %t1052 = add i64 0, 2
  %t1050 = sub nsw i64 %t6, %t1052
  %t1053 = getelementptr [1024 x i64], ptr %t1049, i64 0, i64 %t1050
  %t1054 = load i64, ptr %t1053
  %t1055 = add i64 0, 0
  %t1058 = inttoptr i64 %ac0 to ptr
  %t1059 = getelementptr i8, ptr %t1058, i64 0
  %t1062 = add i64 0, 2
  %t1060 = sub nsw i64 %t6, %t1062
  %t1063 = getelementptr [1024 x i64], ptr %t1059, i64 0, i64 %t1060
  store i64 %t1055, ptr %t1063
  %t1067 = icmp sge i64 %t1054, %t1044
  %t1064 = zext i1 %t1067 to i8
  %t1069 = trunc i8 %t1064 to i1
  br i1 %t1069, label %guard.then1068, label %guard.end1068
  guard.then1068:
  %t1070 = add i64 0, 1
  %t1073 = inttoptr i64 %ac0 to ptr
  %t1074 = getelementptr i8, ptr %t1073, i64 0
  %t1077 = add i64 0, 2
  %t1075 = sub nsw i64 %t6, %t1077
  %t1078 = getelementptr [1024 x i64], ptr %t1074, i64 0, i64 %t1075
  store i64 %t1070, ptr %t1078
  br label %guard.end1068
  guard.end1068:
  %t1082 = add i64 0, 1
  %t1080 = sub nsw i64 %t6, %t1082
  %t1084 = inttoptr i64 %ac0 to ptr
  %t1085 = getelementptr i8, ptr %t1084, i64 8192
  store i64 %t1080, ptr %t1085
  br label %guard.end1033
  guard.end1033:
  %t1089 = add i64 0, 1
  %t1087 = add nsw i64 %arg8, %t1089
  ret i64 %t1087
.smt_body_8_21:
  %t1093 = add i64 0, 1
  %t1094 = icmp sge i64 %t6, %t1093
  %t1091 = zext i1 %t1094 to i8
  %t1096 = trunc i8 %t1091 to i1
  br i1 %t1096, label %guard.then1095, label %guard.end1095
  guard.then1095:
  %t1100 = inttoptr i64 %ac0 to ptr
  %t1101 = getelementptr i8, ptr %t1100, i64 0
  %t1104 = add i64 0, 1
  %t1102 = sub nsw i64 %t6, %t1104
  %t1105 = getelementptr [1024 x i64], ptr %t1101, i64 0, i64 %t1102
  %t1106 = load i64, ptr %t1105
  %t1109 = inttoptr i64 %t1106 to ptr
  %t1107 = load i64, ptr %t1109
  %t1112 = inttoptr i64 %ac0 to ptr
  %t1113 = getelementptr i8, ptr %t1112, i64 0
  %t1116 = add i64 0, 1
  %t1114 = sub nsw i64 %t6, %t1116
  %t1117 = getelementptr [1024 x i64], ptr %t1113, i64 0, i64 %t1114
  store i64 %t1107, ptr %t1117
  br label %guard.end1095
  guard.end1095:
  %t1121 = add i64 0, 1
  %t1119 = add nsw i64 %arg8, %t1121
  ret i64 %t1119
.smt_body_8_22:
  %t1125 = add i64 0, 2
  %t1126 = icmp sge i64 %t6, %t1125
  %t1123 = zext i1 %t1126 to i8
  %t1128 = trunc i8 %t1123 to i1
  br i1 %t1128, label %guard.then1127, label %guard.end1127
  guard.then1127:
  %t1132 = inttoptr i64 %ac0 to ptr
  %t1133 = getelementptr i8, ptr %t1132, i64 0
  %t1136 = add i64 0, 2
  %t1134 = sub nsw i64 %t6, %t1136
  %t1137 = getelementptr [1024 x i64], ptr %t1133, i64 0, i64 %t1134
  %t1138 = load i64, ptr %t1137
  %t1142 = inttoptr i64 %ac0 to ptr
  %t1143 = getelementptr i8, ptr %t1142, i64 0
  %t1146 = add i64 0, 1
  %t1144 = sub nsw i64 %t6, %t1146
  %t1147 = getelementptr [1024 x i64], ptr %t1143, i64 0, i64 %t1144
  %t1148 = load i64, ptr %t1147
  %t1152 = inttoptr i64 %t1138 to ptr
  store i64 %t1148, ptr %t1152
  %t1149 = add i64 0, 0
  %t1155 = add i64 0, 2
  %t1153 = sub nsw i64 %t6, %t1155
  %t1157 = inttoptr i64 %ac0 to ptr
  %t1158 = getelementptr i8, ptr %t1157, i64 8192
  store i64 %t1153, ptr %t1158
  br label %guard.end1127
  guard.end1127:
  %t1162 = add i64 0, 1
  %t1160 = add nsw i64 %arg8, %t1162
  ret i64 %t1160
.smt_body_8_23:
  %t1165 = add i64 0, 1
  %t1164 = sub i64 0, %t1165
  ret i64 %t1164
.smt_body_8_24:
  %t1168 = add i64 0, 1
  %t1167 = sub i64 0, %t1168
  ret i64 %t1167
.smt_body_8_25:
  %t1174 = add i64 0, 1
  %t1172 = add nsw i64 %arg8, %t1174
  %t1175 = inttoptr i64 %ac3 to ptr
  %t1170 = call i64 @read_i8(ptr %state, ptr %t1175, i64 %t1172)
  %t1178 = add i64 0, 1024
  %t1179 = icmp slt i64 %t6, %t1178
  %t1176 = zext i1 %t1179 to i8
  %t1181 = trunc i8 %t1176 to i1
  br i1 %t1181, label %guard.then1180, label %guard.end1180
  guard.then1180:
  %t1185 = inttoptr i64 %ac0 to ptr
  %t1186 = getelementptr i8, ptr %t1185, i64 0
  %t1188 = getelementptr [1024 x i64], ptr %t1186, i64 0, i64 %t6
  store i64 %t1170, ptr %t1188
  %t1191 = add i64 0, 1
  %t1189 = add nsw i64 %t6, %t1191
  %t1193 = inttoptr i64 %ac0 to ptr
  %t1194 = getelementptr i8, ptr %t1193, i64 8192
  store i64 %t1189, ptr %t1194
  br label %guard.end1180
  guard.end1180:
  %t1198 = add i64 0, 2
  %t1196 = add nsw i64 %arg8, %t1198
  ret i64 %t1196
.smt_body_8_26:
  %t1204 = add i64 0, 1
  %t1202 = add nsw i64 %arg8, %t1204
  %t1205 = inttoptr i64 %ac3 to ptr
  %t1200 = call i64 @read_u8(ptr %state, ptr %t1205, i64 %t1202)
  %t1209 = inttoptr i64 %ac1 to ptr
  %t1210 = getelementptr i8, ptr %t1209, i64 0
  %t1211 = add nsw i64 %arg9, %t1200
  %t1214 = getelementptr [4096 x i64], ptr %t1210, i64 0, i64 %t1211
  %t1215 = load i64, ptr %t1214
  %t1218 = add i64 0, 1024
  %t1219 = icmp slt i64 %t6, %t1218
  %t1216 = zext i1 %t1219 to i8
  %t1221 = trunc i8 %t1216 to i1
  br i1 %t1221, label %guard.then1220, label %guard.end1220
  guard.then1220:
  %t1225 = inttoptr i64 %ac0 to ptr
  %t1226 = getelementptr i8, ptr %t1225, i64 0
  %t1228 = getelementptr [1024 x i64], ptr %t1226, i64 0, i64 %t6
  store i64 %t1215, ptr %t1228
  %t1231 = add i64 0, 1
  %t1229 = add nsw i64 %t6, %t1231
  %t1233 = inttoptr i64 %ac0 to ptr
  %t1234 = getelementptr i8, ptr %t1233, i64 8192
  store i64 %t1229, ptr %t1234
  br label %guard.end1220
  guard.end1220:
  %t1238 = add i64 0, 2
  %t1236 = add nsw i64 %arg8, %t1238
  ret i64 %t1236
.smt_body_8_27:
  %t1244 = add i64 0, 1
  %t1242 = add nsw i64 %arg8, %t1244
  %t1245 = inttoptr i64 %ac3 to ptr
  %t1240 = call i64 @read_u8(ptr %state, ptr %t1245, i64 %t1242)
  %t1248 = add i64 0, 1
  %t1249 = icmp sge i64 %t6, %t1248
  %t1246 = zext i1 %t1249 to i8
  %t1251 = trunc i8 %t1246 to i1
  br i1 %t1251, label %guard.then1250, label %guard.end1250
  guard.then1250:
  %t1255 = inttoptr i64 %ac0 to ptr
  %t1256 = getelementptr i8, ptr %t1255, i64 0
  %t1259 = add i64 0, 1
  %t1257 = sub nsw i64 %t6, %t1259
  %t1260 = getelementptr [1024 x i64], ptr %t1256, i64 0, i64 %t1257
  %t1261 = load i64, ptr %t1260
  %t1264 = inttoptr i64 %ac1 to ptr
  %t1265 = getelementptr i8, ptr %t1264, i64 0
  %t1266 = add nsw i64 %arg9, %t1240
  %t1269 = getelementptr [4096 x i64], ptr %t1265, i64 0, i64 %t1266
  store i64 %t1261, ptr %t1269
  %t1272 = add i64 0, 1
  %t1270 = sub nsw i64 %t6, %t1272
  %t1274 = inttoptr i64 %ac0 to ptr
  %t1275 = getelementptr i8, ptr %t1274, i64 8192
  store i64 %t1270, ptr %t1275
  br label %guard.end1250
  guard.end1250:
  %t1279 = add i64 0, 2
  %t1277 = add nsw i64 %arg8, %t1279
  ret i64 %t1277
.smt_body_8_28:
  %t1285 = add i64 0, 1
  %t1283 = add nsw i64 %arg8, %t1285
  %t1286 = inttoptr i64 %ac3 to ptr
  %t1281 = call i64 @read_u8(ptr %state, ptr %t1286, i64 %t1283)
    %t1292 = inttoptr i64 %ac2 to ptr
    %t1293 = getelementptr i8, ptr %t1292, i64 6144
    %t1294 = load i64, ptr %t1293
  %t1295 = add i64 0, 256
  %t1296 = icmp slt i64 %t1294, %t1295
  %t1287 = zext i1 %t1296 to i8
  %t1298 = trunc i8 %t1287 to i1
  br i1 %t1298, label %guard.then1297, label %guard.end1297
  guard.then1297:
    %t1303 = inttoptr i64 %ac1 to ptr
    %t1304 = getelementptr i8, ptr %t1303, i64 32768
    %t1305 = load i64, ptr %t1304
  %t1309 = inttoptr i64 %ac2 to ptr
  %t1310 = getelementptr i8, ptr %t1309, i64 0
    %t1315 = inttoptr i64 %ac2 to ptr
    %t1316 = getelementptr i8, ptr %t1315, i64 6144
    %t1317 = load i64, ptr %t1316
  %t1318 = mul i64 %t1317, 24
  %t1319 = getelementptr i8, ptr %t1310, i64 %t1318
  %t1320 = ptrtoint ptr %t1319 to i64
  %t1321 = inttoptr i64 %t1320 to ptr
  %t1322 = getelementptr i8, ptr %t1321, i64 0
  store i64 %t1305, ptr %t1322
  %t1327 = inttoptr i64 %ac2 to ptr
  %t1328 = getelementptr i8, ptr %t1327, i64 0
    %t1333 = inttoptr i64 %ac2 to ptr
    %t1334 = getelementptr i8, ptr %t1333, i64 6144
    %t1335 = load i64, ptr %t1334
  %t1336 = mul i64 %t1335, 24
  %t1337 = getelementptr i8, ptr %t1328, i64 %t1336
  %t1338 = ptrtoint ptr %t1337 to i64
  %t1339 = inttoptr i64 %t1338 to ptr
  %t1340 = getelementptr i8, ptr %t1339, i64 8
  store i64 %t1281, ptr %t1340
  %t1343 = add i64 0, 2
  %t1341 = add nsw i64 %arg8, %t1343
  %t1347 = inttoptr i64 %ac2 to ptr
  %t1348 = getelementptr i8, ptr %t1347, i64 0
    %t1353 = inttoptr i64 %ac2 to ptr
    %t1354 = getelementptr i8, ptr %t1353, i64 6144
    %t1355 = load i64, ptr %t1354
  %t1356 = mul i64 %t1355, 24
  %t1357 = getelementptr i8, ptr %t1348, i64 %t1356
  %t1358 = ptrtoint ptr %t1357 to i64
  %t1359 = inttoptr i64 %t1358 to ptr
  %t1360 = getelementptr i8, ptr %t1359, i64 16
  store i64 %t1341, ptr %t1360
    %t1366 = inttoptr i64 %ac2 to ptr
    %t1367 = getelementptr i8, ptr %t1366, i64 6144
    %t1368 = load i64, ptr %t1367
  %t1369 = add i64 0, 1
  %t1361 = add nsw i64 %t1368, %t1369
  %t1371 = inttoptr i64 %ac2 to ptr
  %t1372 = getelementptr i8, ptr %t1371, i64 6144
  store i64 %t1361, ptr %t1372
  br label %guard.end1297
  guard.end1297:
  %t1376 = add i64 0, 2
  %t1374 = add nsw i64 %arg8, %t1376
  ret i64 %t1374
.smt_body_8_29:
    %t1383 = inttoptr i64 %ac2 to ptr
    %t1384 = getelementptr i8, ptr %t1383, i64 6144
    %t1385 = load i64, ptr %t1384
  %t1386 = add i64 0, 0
  %t1387 = icmp sgt i64 %t1385, %t1386
  %t1378 = zext i1 %t1387 to i8
  %t1389 = trunc i8 %t1378 to i1
  br i1 %t1389, label %guard.then1388, label %guard.end1388
  guard.then1388:
    %t1395 = inttoptr i64 %ac2 to ptr
    %t1396 = getelementptr i8, ptr %t1395, i64 6144
    %t1397 = load i64, ptr %t1396
  %t1398 = add i64 0, 1
  %t1390 = sub nsw i64 %t1397, %t1398
  %t1400 = inttoptr i64 %ac2 to ptr
  %t1401 = getelementptr i8, ptr %t1400, i64 6144
  store i64 %t1390, ptr %t1401
  br label %guard.end1388
  guard.end1388:
  %t1405 = add i64 0, 1
  %t1403 = add nsw i64 %arg8, %t1405
  ret i64 %t1403
.smt_body_8_30:
  %t1411 = add i64 0, 1
  %t1409 = add nsw i64 %arg8, %t1411
  %t1412 = inttoptr i64 %ac3 to ptr
  %t1407 = call i64 @read_i16(ptr %state, ptr %t1412, i64 %t1409)
  %t1415 = add i64 0, 1024
  %t1416 = icmp slt i64 %t6, %t1415
  %t1413 = zext i1 %t1416 to i8
  %t1418 = trunc i8 %t1413 to i1
  br i1 %t1418, label %guard.then1417, label %guard.end1417
  guard.then1417:
  %t1422 = inttoptr i64 %ac0 to ptr
  %t1423 = getelementptr i8, ptr %t1422, i64 0
  %t1425 = getelementptr [1024 x i64], ptr %t1423, i64 0, i64 %t6
  store i64 %t1407, ptr %t1425
  %t1428 = add i64 0, 1
  %t1426 = add nsw i64 %t6, %t1428
  %t1430 = inttoptr i64 %ac0 to ptr
  %t1431 = getelementptr i8, ptr %t1430, i64 8192
  store i64 %t1426, ptr %t1431
  br label %guard.end1417
  guard.end1417:
  %t1435 = add i64 0, 3
  %t1433 = add nsw i64 %arg8, %t1435
  ret i64 %t1433
.smt_body_8_31:
  %t1441 = add i64 0, 1
  %t1439 = add nsw i64 %arg8, %t1441
  %t1442 = inttoptr i64 %ac3 to ptr
  %t1437 = call i64 @read_i16(ptr %state, ptr %t1442, i64 %t1439)
  %t1446 = add i64 0, 3
  %t1444 = add nsw i64 %arg8, %t1446
  %t1443 = add nsw i64 %t1444, %t1437
  ret i64 %t1443
.smt_body_8_32:
  %t1453 = add i64 0, 1
  %t1451 = add nsw i64 %arg8, %t1453
  %t1454 = inttoptr i64 %ac3 to ptr
  %t1449 = call i64 @read_i16(ptr %state, ptr %t1454, i64 %t1451)
  %t1457 = add i64 0, 1
  %t1458 = icmp sge i64 %t6, %t1457
  %t1455 = zext i1 %t1458 to i8
  %t1460 = trunc i8 %t1455 to i1
  br i1 %t1460, label %guard.then1459, label %guard.end1459
  guard.then1459:
  %t1464 = inttoptr i64 %ac0 to ptr
  %t1465 = getelementptr i8, ptr %t1464, i64 0
  %t1468 = add i64 0, 1
  %t1466 = sub nsw i64 %t6, %t1468
  %t1469 = getelementptr [1024 x i64], ptr %t1465, i64 0, i64 %t1466
  %t1470 = load i64, ptr %t1469
  %t1473 = add i64 0, 1
  %t1471 = sub nsw i64 %t6, %t1473
  %t1475 = inttoptr i64 %ac0 to ptr
  %t1476 = getelementptr i8, ptr %t1475, i64 8192
  store i64 %t1471, ptr %t1476
  %t1479 = add i64 0, 0
  %t1480 = icmp eq i64 %t1470, %t1479
  %t1477 = zext i1 %t1480 to i8
  %t1482 = trunc i8 %t1477 to i1
  br i1 %t1482, label %guard.then1481, label %guard.end1481
  guard.then1481:
  %t1486 = add i64 0, 3
  %t1484 = add nsw i64 %arg8, %t1486
  %t1483 = add nsw i64 %t1484, %t1449
  ret i64 %t1483
  guard.end1481:
  %t1492 = add i64 0, 3
  %t1490 = add nsw i64 %arg8, %t1492
  ret i64 %t1490
  guard.end1459:
  %t1497 = add i64 0, 3
  %t1495 = add nsw i64 %arg8, %t1497
  ret i64 %t1495
.smt_body_8_33:
  %t1503 = add i64 0, 1
  %t1501 = add nsw i64 %arg8, %t1503
  %t1504 = inttoptr i64 %ac3 to ptr
  %t1499 = call i64 @read_i16(ptr %state, ptr %t1504, i64 %t1501)
  %t1507 = add i64 0, 1
  %t1508 = icmp sge i64 %t6, %t1507
  %t1505 = zext i1 %t1508 to i8
  %t1510 = trunc i8 %t1505 to i1
  br i1 %t1510, label %guard.then1509, label %guard.end1509
  guard.then1509:
  %t1514 = inttoptr i64 %ac0 to ptr
  %t1515 = getelementptr i8, ptr %t1514, i64 0
  %t1518 = add i64 0, 1
  %t1516 = sub nsw i64 %t6, %t1518
  %t1519 = getelementptr [1024 x i64], ptr %t1515, i64 0, i64 %t1516
  %t1520 = load i64, ptr %t1519
  %t1523 = add i64 0, 1
  %t1521 = sub nsw i64 %t6, %t1523
  %t1525 = inttoptr i64 %ac0 to ptr
  %t1526 = getelementptr i8, ptr %t1525, i64 8192
  store i64 %t1521, ptr %t1526
  %t1529 = add i64 0, 0
  %t1530 = icmp ne i64 %t1520, %t1529
  %t1527 = zext i1 %t1530 to i8
  %t1532 = trunc i8 %t1527 to i1
  br i1 %t1532, label %guard.then1531, label %guard.end1531
  guard.then1531:
  %t1536 = add i64 0, 3
  %t1534 = add nsw i64 %arg8, %t1536
  %t1533 = add nsw i64 %t1534, %t1499
  ret i64 %t1533
  guard.end1531:
  %t1542 = add i64 0, 3
  %t1540 = add nsw i64 %arg8, %t1542
  ret i64 %t1540
  guard.end1509:
  %t1547 = add i64 0, 3
  %t1545 = add nsw i64 %arg8, %t1547
  ret i64 %t1545
.smt_body_8_34:
  %t1553 = add i64 0, 1
  %t1551 = add nsw i64 %arg8, %t1553
  %t1554 = inttoptr i64 %ac3 to ptr
  %t1549 = call i64 @read_u16(ptr %state, ptr %t1554, i64 %t1551)
  %t1558 = icmp slt i64 %t1549, %arg6
  %t1555 = zext i1 %t1558 to i8
  %t1560 = trunc i8 %t1555 to i1
  br i1 %t1560, label %guard.then1559, label %guard.end1559
  guard.then1559:
  %t1565 = inttoptr i64 %ac4 to ptr
  %t1561 = call i64 @fn_bc_offset(ptr %state, ptr %t1565, i64 %arg5, i64 %t1549)
  %t1570 = inttoptr i64 %ac4 to ptr
  %t1566 = call i64 @fn_local_count(ptr %state, ptr %t1570, i64 %arg5, i64 %t1549)
  %t1575 = inttoptr i64 %ac4 to ptr
  %t1571 = call i64 @fn_arg_count(ptr %state, ptr %t1575, i64 %arg5, i64 %t1549)
    %t1581 = inttoptr i64 %ac2 to ptr
    %t1582 = getelementptr i8, ptr %t1581, i64 6144
    %t1583 = load i64, ptr %t1582
  %t1584 = add i64 0, 256
  %t1585 = icmp slt i64 %t1583, %t1584
  %t1576 = zext i1 %t1585 to i8
  %t1587 = trunc i8 %t1576 to i1
  br i1 %t1587, label %guard.then1586, label %guard.end1586
  guard.then1586:
  %t1590 = add i64 0, 3
  %t1588 = add nsw i64 %arg8, %t1590
  %t1594 = inttoptr i64 %ac2 to ptr
  %t1595 = getelementptr i8, ptr %t1594, i64 0
    %t1600 = inttoptr i64 %ac2 to ptr
    %t1601 = getelementptr i8, ptr %t1600, i64 6144
    %t1602 = load i64, ptr %t1601
  %t1603 = mul i64 %t1602, 24
  %t1604 = getelementptr i8, ptr %t1595, i64 %t1603
  %t1605 = ptrtoint ptr %t1604 to i64
  %t1606 = inttoptr i64 %t1605 to ptr
  %t1607 = getelementptr i8, ptr %t1606, i64 16
  store i64 %t1588, ptr %t1607
    %t1612 = inttoptr i64 %ac1 to ptr
    %t1613 = getelementptr i8, ptr %t1612, i64 32768
    %t1614 = load i64, ptr %t1613
  %t1618 = inttoptr i64 %ac2 to ptr
  %t1619 = getelementptr i8, ptr %t1618, i64 0
    %t1624 = inttoptr i64 %ac2 to ptr
    %t1625 = getelementptr i8, ptr %t1624, i64 6144
    %t1626 = load i64, ptr %t1625
  %t1627 = mul i64 %t1626, 24
  %t1628 = getelementptr i8, ptr %t1619, i64 %t1627
  %t1629 = ptrtoint ptr %t1628 to i64
  %t1630 = inttoptr i64 %t1629 to ptr
  %t1631 = getelementptr i8, ptr %t1630, i64 0
  store i64 %t1614, ptr %t1631
  %t1636 = inttoptr i64 %ac2 to ptr
  %t1637 = getelementptr i8, ptr %t1636, i64 0
    %t1642 = inttoptr i64 %ac2 to ptr
    %t1643 = getelementptr i8, ptr %t1642, i64 6144
    %t1644 = load i64, ptr %t1643
  %t1645 = mul i64 %t1644, 24
  %t1646 = getelementptr i8, ptr %t1637, i64 %t1645
  %t1647 = ptrtoint ptr %t1646 to i64
  %t1648 = inttoptr i64 %t1647 to ptr
  %t1649 = getelementptr i8, ptr %t1648, i64 8
  store i64 %t1566, ptr %t1649
    %t1655 = inttoptr i64 %ac2 to ptr
    %t1656 = getelementptr i8, ptr %t1655, i64 6144
    %t1657 = load i64, ptr %t1656
  %t1658 = add i64 0, 1
  %t1650 = add nsw i64 %t1657, %t1658
  %t1660 = inttoptr i64 %ac2 to ptr
  %t1661 = getelementptr i8, ptr %t1660, i64 6144
  store i64 %t1650, ptr %t1661
  %t1664 = add i64 0, 1
  %t1665 = icmp sge i64 %t1571, %t1664
  %t1662 = zext i1 %t1665 to i8
  %t1667 = trunc i8 %t1662 to i1
  br i1 %t1667, label %guard.then1666, label %guard.end1666
  guard.then1666:
  %t1671 = inttoptr i64 %ac0 to ptr
  %t1672 = getelementptr i8, ptr %t1671, i64 0
  %t1675 = add i64 0, 1
  %t1673 = sub nsw i64 %t6, %t1675
  %t1676 = getelementptr [1024 x i64], ptr %t1672, i64 0, i64 %t1673
  %t1677 = load i64, ptr %t1676
  %t1680 = inttoptr i64 %ac1 to ptr
  %t1681 = getelementptr i8, ptr %t1680, i64 0
    %t1687 = inttoptr i64 %ac1 to ptr
    %t1688 = getelementptr i8, ptr %t1687, i64 32768
    %t1689 = load i64, ptr %t1688
  %t1690 = add i64 0, 0
  %t1682 = add nsw i64 %t1689, %t1690
  %t1691 = getelementptr [4096 x i64], ptr %t1681, i64 0, i64 %t1682
  store i64 %t1677, ptr %t1691
  br label %guard.end1666
  guard.end1666:
  %t1695 = add i64 0, 2
  %t1696 = icmp sge i64 %t1571, %t1695
  %t1693 = zext i1 %t1696 to i8
  %t1698 = trunc i8 %t1693 to i1
  br i1 %t1698, label %guard.then1697, label %guard.end1697
  guard.then1697:
  %t1702 = inttoptr i64 %ac0 to ptr
  %t1703 = getelementptr i8, ptr %t1702, i64 0
  %t1706 = add i64 0, 2
  %t1704 = sub nsw i64 %t6, %t1706
  %t1707 = getelementptr [1024 x i64], ptr %t1703, i64 0, i64 %t1704
  %t1708 = load i64, ptr %t1707
  %t1711 = inttoptr i64 %ac1 to ptr
  %t1712 = getelementptr i8, ptr %t1711, i64 0
    %t1718 = inttoptr i64 %ac1 to ptr
    %t1719 = getelementptr i8, ptr %t1718, i64 32768
    %t1720 = load i64, ptr %t1719
  %t1721 = add i64 0, 1
  %t1713 = add nsw i64 %t1720, %t1721
  %t1722 = getelementptr [4096 x i64], ptr %t1712, i64 0, i64 %t1713
  store i64 %t1708, ptr %t1722
  br label %guard.end1697
  guard.end1697:
  %t1726 = add i64 0, 3
  %t1727 = icmp sge i64 %t1571, %t1726
  %t1724 = zext i1 %t1727 to i8
  %t1729 = trunc i8 %t1724 to i1
  br i1 %t1729, label %guard.then1728, label %guard.end1728
  guard.then1728:
  %t1733 = inttoptr i64 %ac0 to ptr
  %t1734 = getelementptr i8, ptr %t1733, i64 0
  %t1737 = add i64 0, 3
  %t1735 = sub nsw i64 %t6, %t1737
  %t1738 = getelementptr [1024 x i64], ptr %t1734, i64 0, i64 %t1735
  %t1739 = load i64, ptr %t1738
  %t1742 = inttoptr i64 %ac1 to ptr
  %t1743 = getelementptr i8, ptr %t1742, i64 0
    %t1749 = inttoptr i64 %ac1 to ptr
    %t1750 = getelementptr i8, ptr %t1749, i64 32768
    %t1751 = load i64, ptr %t1750
  %t1752 = add i64 0, 2
  %t1744 = add nsw i64 %t1751, %t1752
  %t1753 = getelementptr [4096 x i64], ptr %t1743, i64 0, i64 %t1744
  store i64 %t1739, ptr %t1753
  br label %guard.end1728
  guard.end1728:
  %t1757 = add i64 0, 4
  %t1758 = icmp sge i64 %t1571, %t1757
  %t1755 = zext i1 %t1758 to i8
  %t1760 = trunc i8 %t1755 to i1
  br i1 %t1760, label %guard.then1759, label %guard.end1759
  guard.then1759:
  %t1764 = inttoptr i64 %ac0 to ptr
  %t1765 = getelementptr i8, ptr %t1764, i64 0
  %t1768 = add i64 0, 4
  %t1766 = sub nsw i64 %t6, %t1768
  %t1769 = getelementptr [1024 x i64], ptr %t1765, i64 0, i64 %t1766
  %t1770 = load i64, ptr %t1769
  %t1773 = inttoptr i64 %ac1 to ptr
  %t1774 = getelementptr i8, ptr %t1773, i64 0
    %t1780 = inttoptr i64 %ac1 to ptr
    %t1781 = getelementptr i8, ptr %t1780, i64 32768
    %t1782 = load i64, ptr %t1781
  %t1783 = add i64 0, 3
  %t1775 = add nsw i64 %t1782, %t1783
  %t1784 = getelementptr [4096 x i64], ptr %t1774, i64 0, i64 %t1775
  store i64 %t1770, ptr %t1784
  br label %guard.end1759
  guard.end1759:
    %t1791 = inttoptr i64 %ac1 to ptr
    %t1792 = getelementptr i8, ptr %t1791, i64 32768
    %t1793 = load i64, ptr %t1792
  %t1786 = add nsw i64 %t1793, %t1566
  %t1796 = inttoptr i64 %ac1 to ptr
  %t1797 = getelementptr i8, ptr %t1796, i64 32768
  store i64 %t1786, ptr %t1797
  %t1798 = sub nsw i64 %t6, %t1571
  %t1802 = inttoptr i64 %ac0 to ptr
  %t1803 = getelementptr i8, ptr %t1802, i64 8192
  store i64 %t1798, ptr %t1803
  ret i64 %t1561
  guard.end1586:
  %t1809 = add i64 0, 3
  %t1807 = add nsw i64 %arg8, %t1809
  ret i64 %t1807
  guard.end1559:
  %t1814 = add i64 0, 3
  %t1812 = add nsw i64 %arg8, %t1814
  ret i64 %t1812
.smt_body_8_35:
  %t1820 = add i64 0, 1
  %t1818 = add nsw i64 %arg8, %t1820
  %t1821 = inttoptr i64 %ac3 to ptr
  %t1816 = call i64 @read_i32(ptr %state, ptr %t1821, i64 %t1818)
  %t1824 = add i64 0, 1024
  %t1825 = icmp slt i64 %t6, %t1824
  %t1822 = zext i1 %t1825 to i8
  %t1827 = trunc i8 %t1822 to i1
  br i1 %t1827, label %guard.then1826, label %guard.end1826
  guard.then1826:
  %t1831 = inttoptr i64 %ac0 to ptr
  %t1832 = getelementptr i8, ptr %t1831, i64 0
  %t1834 = getelementptr [1024 x i64], ptr %t1832, i64 0, i64 %t6
  store i64 %t1816, ptr %t1834
  %t1837 = add i64 0, 1
  %t1835 = add nsw i64 %t6, %t1837
  %t1839 = inttoptr i64 %ac0 to ptr
  %t1840 = getelementptr i8, ptr %t1839, i64 8192
  store i64 %t1835, ptr %t1840
  br label %guard.end1826
  guard.end1826:
  %t1844 = add i64 0, 5
  %t1842 = add nsw i64 %arg8, %t1844
  ret i64 %t1842
.smt_body_8_36:
  %t1850 = add i64 0, 1
  %t1848 = add nsw i64 %arg8, %t1850
  %t1851 = inttoptr i64 %ac3 to ptr
  %t1846 = call i64 @read_u32(ptr %state, ptr %t1851, i64 %t1848)
   %t1852 = call i64 @briev_host_arity_of(i64 %t1846)
  %t1856 = add i64 0, 1
  %t1857 = icmp eq i64 %t1852, %t1856
  %t1854 = zext i1 %t1857 to i8
  %t1859 = trunc i8 %t1854 to i1
  br i1 %t1859, label %guard.then1858, label %guard.end1858
  guard.then1858:
  %t1863 = inttoptr i64 %ac0 to ptr
  %t1864 = getelementptr i8, ptr %t1863, i64 0
    %t1870 = inttoptr i64 %ac0 to ptr
    %t1871 = getelementptr i8, ptr %t1870, i64 8192
    %t1872 = load i64, ptr %t1871
  %t1873 = add i64 0, 1
  %t1865 = sub nsw i64 %t1872, %t1873
  %t1874 = getelementptr [1024 x i64], ptr %t1864, i64 0, i64 %t1865
  %t1875 = load i64, ptr %t1874
    %t1881 = inttoptr i64 %ac0 to ptr
    %t1882 = getelementptr i8, ptr %t1881, i64 8192
    %t1883 = load i64, ptr %t1882
  %t1884 = add i64 0, 1
  %t1876 = sub nsw i64 %t1883, %t1884
  %t1886 = inttoptr i64 %ac0 to ptr
  %t1887 = getelementptr i8, ptr %t1886, i64 8192
  store i64 %t1876, ptr %t1887
  %t1888 = call i64 @host_dispatch1(ptr %state, i64 %t1846, i64 %t1875)
  br label %guard.end1858
  guard.end1858:
  %t1894 = add i64 0, 5
  %t1892 = add nsw i64 %arg8, %t1894
  ret i64 %t1892
.smt_body_8_37:
  %t1900 = add i64 0, 1
  %t1898 = add nsw i64 %arg8, %t1900
  %t1901 = inttoptr i64 %ac3 to ptr
  %t1896 = call i64 @read_i64(ptr %state, ptr %t1901, i64 %t1898)
  %t1904 = add i64 0, 1024
  %t1905 = icmp slt i64 %t6, %t1904
  %t1902 = zext i1 %t1905 to i8
  %t1907 = trunc i8 %t1902 to i1
  br i1 %t1907, label %guard.then1906, label %guard.end1906
  guard.then1906:
  %t1911 = inttoptr i64 %ac0 to ptr
  %t1912 = getelementptr i8, ptr %t1911, i64 0
  %t1914 = getelementptr [1024 x i64], ptr %t1912, i64 0, i64 %t6
  store i64 %t1896, ptr %t1914
  %t1917 = add i64 0, 1
  %t1915 = add nsw i64 %t6, %t1917
  %t1919 = inttoptr i64 %ac0 to ptr
  %t1920 = getelementptr i8, ptr %t1919, i64 8192
  store i64 %t1915, ptr %t1920
  br label %guard.end1906
  guard.end1906:
  %t1924 = add i64 0, 9
  %t1922 = add nsw i64 %arg8, %t1924
  ret i64 %t1922
.smt_body_8_38:
  %t1928 = add i64 0, 1
  %t1929 = icmp sge i64 %t6, %t1928
  %t1926 = zext i1 %t1929 to i8
  %t1931 = trunc i8 %t1926 to i1
  br i1 %t1931, label %guard.then1930, label %guard.end1930
  guard.then1930:
  %t1936 = inttoptr i64 %ac0 to ptr
  %t1937 = getelementptr i8, ptr %t1936, i64 0
  %t1940 = add i64 0, 1
  %t1938 = sub nsw i64 %t6, %t1940
  %t1941 = getelementptr [1024 x i64], ptr %t1937, i64 0, i64 %t1938
  %t1942 = load i64, ptr %t1941
  %t1932 = xor i64 %t1942, -1
  %t1945 = inttoptr i64 %ac0 to ptr
  %t1946 = getelementptr i8, ptr %t1945, i64 0
  %t1949 = add i64 0, 1
  %t1947 = sub nsw i64 %t6, %t1949
  %t1950 = getelementptr [1024 x i64], ptr %t1946, i64 0, i64 %t1947
  store i64 %t1932, ptr %t1950
  br label %guard.end1930
  guard.end1930:
  %t1954 = add i64 0, 1
  %t1952 = add nsw i64 %arg8, %t1954
  ret i64 %t1952
.smt_body_8_39:
  %t1957 = add i64 0, 1
  %t1956 = sub i64 0, %t1957
  ret i64 %t1956
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
