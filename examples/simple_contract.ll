; ModuleID = 'program.ll'
source_filename = "program.bv"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

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
declare i64 @__read_file__(i64)
declare i64 @__write_file__(i64, i64)
declare i64 @__readln__()
declare i64 @__sort_list__(i64)
declare i64 @__reverse_list__(i64)
declare i64 @__range__(i64)
declare i64 @__trim_left__(ptr)
declare i64 @__trim_right__(ptr)
declare i64 @__to_lower__(ptr)
declare i64 @__contains_at__(ptr, ptr, i64)
declare i64 @__find_from__(ptr, ptr, i64)
declare i64 @__splitn__(ptr, ptr, i64)
declare i64 @__float_to_str(float)
declare i64 @__to_str(i64)
declare i64 @__stack_top__(i64)
declare i64 @briv_syscall(i64, i64, i64, i64, i64, i64, i64)
declare i64 @briv_sysconf(i64)
declare ptr @dlopen(ptr, i32) nounwind
declare ptr @dlsym(ptr, ptr) nounwind
declare i32 @dlclose(ptr) nounwind
declare i64 @briv_backtrace()
declare i64 @__queue_front__(i64)
declare i64 @__hashmap_get__(i64, i64)
declare i64 @__hashset_elements__(i64)
declare void @__exit()
declare i64 @__tty_raw_mode__(i64)
declare i64 @__spawn_with_output__(i64)
declare i64 @__readlink__(i64)
declare i64 @__getcwd__()
declare i64 @__readdir__(i64)
declare i64 @__sigaction__(i64, i64)
declare i64 @__sigprocmask__(i64, i64)
declare i64 @__getaddrinfo__(i64, i64)
declare i64 @__map_keys__(i64)
declare i64 @__map_values__(i64)
declare i64 @__errno__()
declare i64 @__getrandom__(i64, i64, i64)
declare i64 @__uname__()
declare i64 @__hostname__()
declare i64 @__strerror__(i64)
declare i64 @__strsignal__(i64)
declare i64 @__realpath__(i64)
declare i64 @__backtrace__()
declare i64 @__getpwuid__(i64)
declare i64 @__getgrgid__(i64)
declare i64 @__thread_create__(i64, i64)
declare i64 @__thread_join__(i64)
declare void @__thread_exit__(i64)
declare i64 @__mutex_lock__(i64)
declare i64 @__mutex_unlock__(i64)
declare i64 @__condvar_wait__(i64, i64)
declare i64 @__condvar_signal__(i64)
declare i64 @__condvar_broadcast__(i64)
declare i64 @__getrlimit__(i64)
declare i64 @__setrlimit__(i64, i64)
declare i64 @__mkstemp__(i64)
declare i64 @__mkdtemp__(i64)
declare i64 @__dlopen__(i64)
declare i64 @__dlsym__(i64, i64)
declare i64 @__dlclose__(i64)
declare i64 @__ttyname__(i64)
declare i64 @__print_float(float) #6
declare i64 @__print_char(i64) #6
declare i64 @__getenv_int(ptr) #6
declare i64 @__print_int(i64) #6
declare void @__print_str(ptr) #6
declare ptr @__getenv_briv(ptr) #6
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
%StateChunk0 = type { i64, i64, i64, i64, i64 }
%State = type { i64, i64, i64, i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant <{ i64, [1 x i8] }> <{ i64 0, [1 x i8] c"\00" }>, align 8

@ll_empty_list = private unnamed_addr constant { i64, i64 } { i64 0, i64 0 }

define i64 @file_close(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 3, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_read(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 0, i64 %arg0, i64 %ac1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_write(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 1, i64 %arg0, i64 %ac1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_lseek(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 8, i64 %arg0, i64 %arg1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_ftruncate(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 77, i64 %arg0, i64 %arg1, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_fsync(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 74, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_dup(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 32, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_dup2(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 33, i64 %arg0, i64 %arg1, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_fcntl(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 72, i64 %arg0, i64 %arg1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @close(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t0 = call i64 @file_close(ptr %state, i64 %arg0)
  %t2 = add i64 0, 0
  ret i64 %t2
}

define i64 @socket(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 41, i64 %arg0, i64 %arg1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @bind(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 49, i64 %arg0, i64 %ac1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @listen(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 50, i64 %arg0, i64 %arg1, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @accept(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, ptr %arg2) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 43, i64 %arg0, i64 %ac1, i64 %ac2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @connect(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 42, i64 %arg0, i64 %ac1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @send(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2, i64 %arg3) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 44, i64 %arg0, i64 %ac1, i64 %arg2, i64 %arg3, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @recv(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2, i64 %arg3) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 45, i64 %arg0, i64 %ac1, i64 %arg2, i64 %arg3, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @sendto(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2, i64 %arg3, ptr %arg4, i64 %arg5) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %ac4 = ptrtoint ptr %arg4 to i64
  %t0 = call i64 @briv_syscall(i64 44, i64 %arg0, i64 %ac1, i64 %arg2, i64 %arg3, i64 %ac4, i64 %arg5)
  ret i64 %t0
}

define i64 @recvfrom(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2, i64 %arg3, ptr %arg4, ptr %arg5) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %ac4 = ptrtoint ptr %arg4 to i64
  %ac5 = ptrtoint ptr %arg5 to i64
  %t0 = call i64 @briv_syscall(i64 45, i64 %arg0, i64 %ac1, i64 %arg2, i64 %arg3, i64 %ac4, i64 %ac5)
  ret i64 %t0
}

define i64 @setsockopt(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1, i64 %arg2, ptr %arg3, i64 %arg4) local_unnamed_addr #8 {
  entry:
  %ac3 = ptrtoint ptr %arg3 to i64
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 54, i64 %arg0, i64 %arg1, i64 %arg2, i64 %ac3, i64 %arg4, i64 %t6)
  ret i64 %t0
}

define i64 @getsockopt(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1, i64 %arg2, ptr %arg3, ptr %arg4) local_unnamed_addr #8 {
  entry:
  %ac3 = ptrtoint ptr %arg3 to i64
  %ac4 = ptrtoint ptr %arg4 to i64
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 55, i64 %arg0, i64 %arg1, i64 %arg2, i64 %ac3, i64 %ac4, i64 %t6)
  ret i64 %t0
}

define i64 @shutdown(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 48, i64 %arg0, i64 %arg1, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @sigaction(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, ptr %arg2) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 13, i64 %arg0, i64 %ac1, i64 %ac2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @sigprocmask(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, ptr %arg2) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 14, i64 %arg0, i64 %ac1, i64 %ac2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @pipe(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 22, i64 %ac0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @sem_wait(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 65, i64 %ac0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @sem_post(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = add i64 0, 1
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 65, i64 %ac0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @thread_create(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %t1 = add i64 0, 0
  %t2 = add i64 0, 65536
  %t3 = add i64 0, 3
  %t4 = add i64 0, 34
  %t6 = add i64 0, 1
  %t5 = sub i64 0, %t6
  %t7 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 9, i64 %t1, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t7)
  %t8 = add i64 0, 0
  %t11 = add i64 0, 65536
  %t9 = add nsw i64 %t0, %t11
  %t12 = add i64 0, 4001536
  %t18 = add i64 0, 0
  %t19 = add i64 0, 0
  %t13 = call i64 @briv_syscall(i64 56, i64 %t12, i64 %t9, i64 %arg0, i64 %arg1, i64 %t18, i64 %t19)
  ret i64 %t13
}

define i64 @thread_join(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t1 = add i64 0, 0
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 39, i64 %t1, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @thread_exit(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 60, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @mutex_lock(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 202, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @mutex_unlock(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 1
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 202, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @condvar_wait(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 202, i64 %arg0, i64 %t2, i64 %arg1, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @condvar_signal(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 1
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 202, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @condvar_broadcast(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 2
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 202, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @getuid(ptr noalias nocapture align 8 %state) local_unnamed_addr #8 {
  entry:
  %t1 = add i64 0, 0
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 102, i64 %t1, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @geteuid(ptr noalias nocapture align 8 %state) local_unnamed_addr #8 {
  entry:
  %t1 = add i64 0, 0
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 107, i64 %t1, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @getgid(ptr noalias nocapture align 8 %state) local_unnamed_addr #8 {
  entry:
  %t1 = add i64 0, 0
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 104, i64 %t1, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @getegid(ptr noalias nocapture align 8 %state) local_unnamed_addr #8 {
  entry:
  %t1 = add i64 0, 0
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 108, i64 %t1, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @mmap(ptr noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2, i64 %arg3, i64 %arg4, i64 %arg5) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t0 = call i64 @briv_syscall(i64 9, i64 %ac0, i64 %arg1, i64 %arg2, i64 %arg3, i64 %arg4, i64 %arg5)
  ret i64 %t0
}

define i64 @munmap(ptr noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 11, i64 %ac0, i64 %arg1, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @mprotect(ptr noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 10, i64 %ac0, i64 %arg1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @brk(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 12, i64 %ac0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @mlock(ptr noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @briv_syscall(i64 149, i64 %ac0, i64 %arg1, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @atomic_load(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = inttoptr i64 %ac0 to ptr
  %t0 = load atomic i64, ptr %t2 seq_cst, align 8
  ret i64 %t0
}

define i64 @atomic_store(ptr noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = inttoptr i64 %ac0 to ptr
  store atomic i64 %arg1, ptr %t2 seq_cst, align 8
  %t0 = add i64 0, 0
  %t4 = add i64 0, 0
  ret i64 %t4
}

define i64 @atomic_cas(ptr noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = inttoptr i64 %ac0 to ptr
  %t5 = cmpxchg ptr %t2, i64 %arg1, i64 %arg2 seq_cst seq_cst
  %t0 = extractvalue { i64, i1 } %t5, 0
  ret i64 %t0
}

define i64 @atomic_xchg(ptr noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = inttoptr i64 %ac0 to ptr
  %t0 = atomicrmw xchg ptr %t2, i64 %arg1 seq_cst
  ret i64 %t0
}

define i64 @atomic_add(ptr noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = inttoptr i64 %ac0 to ptr
  %t0 = atomicrmw add ptr %t2, i64 %arg1 seq_cst
  ret i64 %t0
}

define i64 @fence(ptr noalias nocapture align 8 %state) local_unnamed_addr #8 {
  entry:
  fence seq_cst
  %t0 = add i64 0, 0
  %t1 = add i64 0, 0
  ret i64 %t1
}

define i64 @futex(ptr noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2, ptr %arg3, ptr %arg4, i64 %arg5) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac3 = ptrtoint ptr %arg3 to i64
  %ac4 = ptrtoint ptr %arg4 to i64
  %t0 = call i64 @briv_syscall(i64 202, i64 %ac0, i64 %arg1, i64 %arg2, i64 %ac3, i64 %ac4, i64 %arg5)
  ret i64 %t0
}

define ptr @get_env(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
   %t0 = call ptr @__getenv_briv(i64 %ac0)
  ret ptr %t0
}

define i64 @get_env_int(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
   %t0 = call i64 @__getenv_int(i64 %ac0)
  ret i64 %t0
}

define void @txn_increment(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %arinit3 = call ptr @malloc(i64 65536)
  %arii4 = ptrtoint ptr %arinit3 to i64
  %t5 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 %arii4, ptr %t5, align 8
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 %arii4, ptr %t6, align 8
  %arieu7 = getelementptr i8, ptr %arinit3, i64 65536
  %arie8 = ptrtoint ptr %arieu7 to i64
  %t9 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 %arie8, ptr %t9, align 8
  %t12 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t13 = load i64, ptr %t12, align 8
  %t14 = add i64 0, 10
  %t15 = icmp slt i64 %t13, %t14
  %t10 = zext i1 %t15 to i8
  %pi16 = trunc i8 %t10 to i1
  br i1 %pi16, label %ps18, label %pp17
  pp17:
    unreachable
  ps18:
  %t20 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %prl21 = load i64, ptr %t20, align 8, !tbaa !1, !range !{i64 0, i64 10}
  %t24 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t25 = load i64, ptr %t24, align 8
  %t26 = add i64 0, 1
  %t22 = add nsw i64 %t25, %t26
  %t27 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t22, ptr %t27
  ret void
}

define internal i8 @pre_increment(ptr noalias nocapture align 8 %state) #10 {
  entry:
  %t2 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = load i64, ptr %t2, align 8
  %t4 = add i64 0, 10
  %t5 = icmp slt i64 %t3, %t4
  %t0 = zext i1 %t5 to i8
  ret i8 %t0
}
define void @init_state(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 {
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
  ret void
}


define i32 @main() local_unnamed_addr #9 {
entry:
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
  %cmb5 = add i64 0, 1
  %t6 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t7 = load i64, ptr %t6, align 8
  %t8 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t9 = load i64, ptr %t8, align 8
  br label %.cm_header
.cm_header:
  %cmc13 = phi i64 [ %t7, %entry ], [ %cmn10, %.cm_latch ]
  %cmd14 = icmp slt i64 %cmc13, %cmb5
  br i1 %cmd14, label %.cm_body, label %.cm_end_12
.cm_body:
  %t17 = add i64 0, 1
  %t15 = add nsw i64 %cmc13, %t17
  br label %.cm_latch
.cm_latch:
  %cmn10 = add nuw nsw i64 %cmc13, 1
  br label %.cm_header, !llvm.loop !100
.cm_end_12:
  %t18 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %cmc13, ptr %t18, align 8
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

!50 = !{ i64 -9223372036854775808, i64 10 }

!0 = !{!"Briv"}
!1 = !{!"Int", !0}
!2 = !{!"Char", !0}
!3 = !{!"Data", !0}
!4 = !{!"Double", !0}
!5 = !{!"Float", !0}
!6 = !{!"Float32", !0}
!7 = !{!"Float64", !0}
!8 = !{!"Bool", !0}
!9 = !{!"Int16", !0}
!10 = !{!"Int32", !0}
!11 = !{!"Int64", !0}
!12 = !{!"Int8", !0}
!13 = !{!"String", !0}
!14 = !{!"UInt", !0}
!15 = !{!"UInt16", !0}
!16 = !{!"UInt32", !0}
!17 = !{!"UInt64", !0}
!18 = !{!"UInt8", !0}
!19 = !{!"Void", !0}
!99 = distinct !{} ; StateAliasScope
