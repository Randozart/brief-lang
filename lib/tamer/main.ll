; ModuleID = 'program.ll'
source_filename = "program.bv"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

%SmallString64 = type { i64, i64, i64, i64, i64, i64, i64, i64, i64 }
%StaticString = type { i64, i64 }
%String = type { i64, i64 }
%Utf8View = type { i64, i64 }

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
declare i64 @brief_syscall(i64, i64, i64, i64, i64, i64, i64)
declare i64 @brief_sysconf(i64)
declare ptr @dlopen(ptr, i32) nounwind
declare ptr @dlsym(ptr, ptr) nounwind
declare i32 @dlclose(ptr) nounwind
declare i64 @brief_backtrace()
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
declare ptr @__getenv_brief(ptr) #6
declare void @__print_str(ptr) #6
declare i64 @__print_int(i64) #6
declare i64 @__print_float(float) #6
declare i64 @__print_char(i64) #6
declare i64 @__getenv_int(ptr) #6
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
%StateChunk0 = type { i64, i64, i64, i64 }
%State = type { i64, i64, i64, i64 }
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
  %t0 = call i64 @brief_syscall(i64 3, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_read(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 0, i64 %arg0, i64 %ac1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_write(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 1, i64 %arg0, i64 %ac1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_lseek(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 8, i64 %arg0, i64 %arg1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_ftruncate(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 77, i64 %arg0, i64 %arg1, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_fsync(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 74, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_dup(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 32, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_dup2(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 33, i64 %arg0, i64 %arg1, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @file_fcntl(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 72, i64 %arg0, i64 %arg1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i8 @close(ptr noalias nocapture align 8 %state, i8 %arg0) local_unnamed_addr #8 {
  entry:
  %t0 = call i8 @file_close(ptr %state, i8 %arg0)
  %t2 = add i64 0, 0
  ret i8 %t2
}

define i64 @socket(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 41, i64 %arg0, i64 %arg1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @bind(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 49, i64 %arg0, i64 %ac1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @listen(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 50, i64 %arg0, i64 %arg1, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @accept(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, ptr %arg2) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 43, i64 %arg0, i64 %ac1, i64 %ac2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @connect(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 42, i64 %arg0, i64 %ac1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @send(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2, i64 %arg3) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 44, i64 %arg0, i64 %ac1, i64 %arg2, i64 %arg3, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @recv(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2, i64 %arg3) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 45, i64 %arg0, i64 %ac1, i64 %arg2, i64 %arg3, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @sendto(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2, i64 %arg3, ptr %arg4, i64 %arg5) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %ac4 = ptrtoint ptr %arg4 to i64
  %t0 = call i64 @brief_syscall(i64 44, i64 %arg0, i64 %ac1, i64 %arg2, i64 %arg3, i64 %ac4, i64 %arg5)
  ret i64 %t0
}

define i64 @recvfrom(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, i64 %arg2, i64 %arg3, ptr %arg4, ptr %arg5) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %ac4 = ptrtoint ptr %arg4 to i64
  %ac5 = ptrtoint ptr %arg5 to i64
  %t0 = call i64 @brief_syscall(i64 45, i64 %arg0, i64 %ac1, i64 %arg2, i64 %arg3, i64 %ac4, i64 %ac5)
  ret i64 %t0
}

define i64 @setsockopt(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1, i64 %arg2, ptr %arg3, i64 %arg4) local_unnamed_addr #8 {
  entry:
  %ac3 = ptrtoint ptr %arg3 to i64
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 54, i64 %arg0, i64 %arg1, i64 %arg2, i64 %ac3, i64 %arg4, i64 %t6)
  ret i64 %t0
}

define i64 @getsockopt(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1, i64 %arg2, ptr %arg3, ptr %arg4) local_unnamed_addr #8 {
  entry:
  %ac3 = ptrtoint ptr %arg3 to i64
  %ac4 = ptrtoint ptr %arg4 to i64
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 55, i64 %arg0, i64 %arg1, i64 %arg2, i64 %ac3, i64 %ac4, i64 %t6)
  ret i64 %t0
}

define i64 @shutdown(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 48, i64 %arg0, i64 %arg1, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @sigaction(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, ptr %arg2) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 13, i64 %arg0, i64 %ac1, i64 %ac2, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @sigprocmask(ptr noalias nocapture align 8 %state, i64 %arg0, ptr %arg1, ptr %arg2) local_unnamed_addr #8 {
  entry:
  %ac1 = ptrtoint ptr %arg1 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 14, i64 %arg0, i64 %ac1, i64 %ac2, i64 %t4, i64 %t5, i64 %t6)
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
  %t0 = call i64 @brief_syscall(i64 22, i64 %ac0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
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
  %t0 = call i64 @brief_syscall(i64 65, i64 %ac0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
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
  %t0 = call i64 @brief_syscall(i64 65, i64 %ac0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i32 @thread_create(ptr noalias nocapture align 8 %state, i32 %arg0, i32 %arg1) local_unnamed_addr #8 {
  entry:
  %t1 = add i64 0, 0
  %t2 = add i64 0, 65536
  %t3 = add i64 0, 3
  %t4 = add i64 0, 34
  %t6 = add i64 0, 1
  %t5 = sub i64 0, %t6
  %t7 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 9, i64 %t1, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t7)
  %t8 = add i64 0, 0
  %t11 = add i64 0, 65536
  %t9 = add nsw i32 %t0, %t11
  %t12 = add i64 0, 4001536
  %t18 = add i64 0, 0
  %t19 = add i64 0, 0
  %t13 = call i64 @brief_syscall(i64 56, i64 %t12, i64 %t9, i64 %arg0, i64 %arg1, i64 %t18, i64 %t19)
  ret i32 %t13
}

define i64 @thread_join(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t1 = add i64 0, 0
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 39, i64 %t1, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @thread_exit(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 60, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @mutex_lock(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 0
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 202, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @mutex_unlock(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 1
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 202, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @condvar_wait(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 202, i64 %arg0, i64 %t2, i64 %arg1, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @condvar_signal(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 1
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 202, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @condvar_broadcast(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, 2
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 202, i64 %arg0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
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
  %t0 = call i64 @brief_syscall(i64 102, i64 %t1, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
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
  %t0 = call i64 @brief_syscall(i64 107, i64 %t1, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
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
  %t0 = call i64 @brief_syscall(i64 104, i64 %t1, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
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
  %t0 = call i64 @brief_syscall(i64 108, i64 %t1, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @mmap(ptr noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2, i64 %arg3, i64 %arg4, i64 %arg5) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t0 = call i64 @brief_syscall(i64 9, i64 %ac0, i64 %arg1, i64 %arg2, i64 %arg3, i64 %arg4, i64 %arg5)
  ret i64 %t0
}

define i64 @munmap(ptr noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 11, i64 %ac0, i64 %arg1, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @mprotect(ptr noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 10, i64 %ac0, i64 %arg1, i64 %arg2, i64 %t4, i64 %t5, i64 %t6)
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
  %t0 = call i64 @brief_syscall(i64 12, i64 %ac0, i64 %t2, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @mlock(ptr noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t3 = add i64 0, 0
  %t4 = add i64 0, 0
  %t5 = add i64 0, 0
  %t6 = add i64 0, 0
  %t0 = call i64 @brief_syscall(i64 149, i64 %ac0, i64 %arg1, i64 %t3, i64 %t4, i64 %t5, i64 %t6)
  ret i64 %t0
}

define i64 @atomic_load(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = inttoptr i64 %ac0 to ptr
  %t0 = load atomic i64, ptr %t2 seq_cst, align 8
  ret i64 %t0
}

define i8 @atomic_store(ptr noalias nocapture align 8 %state, ptr %arg0, i8 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = inttoptr i64 %ac0 to ptr
  store atomic i64 %arg1, ptr %t2 seq_cst, align 8
  %t0 = add i64 0, 0
  %t4 = add i64 0, 0
  ret i8 %t4
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

define i8 @fence(ptr noalias nocapture align 8 %state) local_unnamed_addr #8 {
  entry:
  fence seq_cst
  %t0 = add i64 0, 0
  %t1 = add i64 0, 0
  ret i8 %t1
}

define i64 @futex(ptr noalias nocapture align 8 %state, ptr %arg0, i64 %arg1, i64 %arg2, ptr %arg3, ptr %arg4, i64 %arg5) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac3 = ptrtoint ptr %arg3 to i64
  %ac4 = ptrtoint ptr %arg4 to i64
  %t0 = call i64 @brief_syscall(i64 202, i64 %ac0, i64 %arg1, i64 %arg2, i64 %ac3, i64 %ac4, i64 %arg5)
  ret i64 %t0
}

define ptr @get_env(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
   %t0 = call ptr @__getenv_brief(i64 %ac0)
  ret ptr %t0
}

define i64 @get_env_int(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
   %t0 = call i64 @__getenv_int(i64 %ac0)
  ret i64 %t0
}

define i8 @tame(ptr %arg0, i8 %arg1, ptr %arg2, i8 %arg3) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac2 = ptrtoint ptr %arg2 to i64
  %t2 = add i64 0, 8192
  %t1_p = call ptr @malloc(i64 %t2)
  %t1 = ptrtoint ptr %t1_p to i64
   %t3 = add i64 %t2, 0
  %t0 = inttoptr i64 %t1 to ptr
  %t6 = add i64 0, 32768
  %t5_p = call ptr @malloc(i64 %t6)
  %t5 = ptrtoint ptr %t5_p to i64
   %t7 = add i64 %t6, 0
  %t4 = inttoptr i64 %t5 to ptr
  %t10 = add i64 0, 8192
  %t9_p = call ptr @malloc(i64 %t10)
  %t9 = ptrtoint ptr %t9_p to i64
   %t11 = add i64 %t10, 0
  %t8 = inttoptr i64 %t9 to ptr
  %t14 = add i64 0, 0
  %t12 = call i8 @read_u32(ptr %ac0, i8 %t14)
  %t17 = add i64 0, 1380532556
  %t18 = icmp ne i8 %t12, %t17
  %t15 = zext i1 %t18 to i8
  %t20 = trunc i8 %t15 to i1
  br i1 %t20, label %guard.then19, label %guard.end19
  guard.then19:
  %t21 = add i64 0, 1
  ret i8 %t21
  br label %guard.end19
  guard.end19:
  %t24 = call i8 @lair_bc_offset(ptr %ac0)
  %t26 = call i8 @lair_bc_size(ptr %ac0)
  %t28 = call i8 @lair_fn_offset(ptr %ac0)
  %t30 = call i8 @lair_fn_size(ptr %ac0)
  %t34 = add i64 0, 20
  %t32 = sdiv i8 %t30, %t34
  %t35 = add nsw i64 %ac0, %t24
  %t38 = add nsw i64 %ac0, %t28
  %t42 = add nsw i8 %t24, %t26
  %t46 = icmp sgt i8 %t42, %arg1
  %t41 = zext i1 %t46 to i8
  %t48 = trunc i8 %t41 to i1
  br i1 %t48, label %guard.then47, label %guard.end47
  guard.then47:
  %t49 = add i64 0, 1
  ret i8 %t49
  br label %guard.end47
  guard.end47:
  %t53 = add nsw i8 %t28, %t30
  %t57 = icmp sgt i8 %t53, %arg1
  %t52 = zext i1 %t57 to i8
  %t59 = trunc i8 %t52 to i1
  br i1 %t59, label %guard.then58, label %guard.end58
  guard.then58:
  %t60 = add i64 0, 1
  ret i8 %t60
  br label %guard.end58
  guard.end58:
  %t65 = add i64 0, 0
  %t66 = icmp eq i8 %t32, %t65
  %t63 = zext i1 %t66 to i8
  %t68 = trunc i8 %t63 to i1
  br i1 %t68, label %guard.then67, label %guard.end67
  guard.then67:
  %t69 = add i64 0, 1
  ret i8 %t69
  br label %guard.end67
  guard.end67:
  %t74 = add i64 0, 0
  %t76 = add i64 0, 0
  %t78 = add i64 0, 0
  %t83 = add i64 0, 0
  %t84 = add i64 0, 0
  %t85 = add i64 0, 1
  %t72 = call i8 @vm_loop(ptr %t0, i8 %t74, ptr %t4, i8 %t76, ptr %t8, i8 %t78, i8 %t35, i8 %t26, i8 %t38, i8 %t32, i8 %t83, i8 %t84, i8 %t85)
  %t86 = add i64 0, 0
  ret i8 %t86
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
  ret void
}


define void @reactor_tick(ptr noalias nocapture %state) local_unnamed_addr #2 {
  entry:
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
  %state_save = alloca %State, align 8
  br label %.loop
.loop:
  call void @llvm.memcpy.p0p0i64(ptr %state_save, ptr %state, i64 32, i1 false)
  call void @reactor_tick(ptr noalias nocapture %state)
  br label %.end
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

!0 = !{!"Brief"}
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
!13 = !{!"SmallString64", !0}
!14 = !{!"StaticString", !0}
!15 = !{!"String", !0}
!16 = !{!"UInt", !0}
!17 = !{!"UInt16", !0}
!18 = !{!"UInt32", !0}
!19 = !{!"UInt64", !0}
!20 = !{!"UInt8", !0}
!21 = !{!"Utf8View", !0}
!22 = !{!"Void", !0}
!99 = distinct !{} ; StateAliasScope
