; ModuleID = 'program.ll'
source_filename = "program.bv"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

%String = type { i64, i64, i64 }
%StringBuilder = type { i64 }

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
declare i32 @ioctl(i32, i64, ptr)
declare i32 @socket(i32, i32, i32)
declare i32 @bind(i32, ptr, i32)
declare i32 @listen(i32, i32)
declare i32 @accept(i32, ptr, ptr)
declare i32 @connect(i32, ptr, i32)
declare i64 @send(i32, ptr, i64, i32)
declare i64 @recv(i32, ptr, i64, i32)
declare i64 @sendto(i32, ptr, i64, i32, ptr, i32)
declare i64 @recvfrom(i32, ptr, i64, i32, ptr, ptr)
declare i32 @setsockopt(i32, i32, i32, ptr, i32)
declare i32 @getsockopt(i32, i32, i32, ptr, ptr)
declare i64 @strlen(i8*) #1
declare i32 @epoll_create1(i32) #1
declare i32 @epoll_ctl(i32, i32, i32, ptr) #1
declare i32 @epoll_wait(i32, ptr, i32, i32) #1
declare i64 @read(i32, ptr, i64) #1
declare i32 @fcntl(i32, i32, i32) #1
declare i32 @timerfd_create(i32, i32) #1
declare i32 @timerfd_settime(i32, i32, ptr, ptr) #1
declare i32 @signalfd(i32, ptr, i32) #1
declare i32 @sigemptyset(i8*) #1
declare i32 @sigaddset(i8*, i32) #1
declare i32 @sigprocmask(i32, ptr, ptr) #1
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
@FMT_INT = private unnamed_addr constant [5 x i8] c"%ld\0A\00"
@FMT_FLOAT = private unnamed_addr constant [6 x i8] c"%.9f\0A\00"
@FMT_STR = private unnamed_addr constant [4 x i8] c"%s\0A\00"
@STR_READFILE_ERR = private unnamed_addr constant [15 x i8] c"file not found\00"
@stdout = external dso_local global ptr
declare i32 @fprintf(ptr, ptr, ...) #1
declare i32 @fputc(i32, ptr) #1
declare i32 @fflush(ptr) #1
declare ptr @getenv(ptr) #1
declare i64 @atol(ptr) #1
declare void @exit(i32) #1
declare void @abort() #1
declare i64 @sysconf(i32) #1
declare i32 @sched_yield() #1
declare i32 @getpriority(i32, i32) #1
declare i32 @setpriority(i32, i32, i32) #1
declare i32 @getuid() #1
declare i32 @geteuid() #1
declare i32 @getgid() #1
declare i32 @getegid() #1
declare i32 @setvbuf(ptr, ptr, i32, i64) #1
declare i32 @pthread_create(ptr, ptr, ptr, ptr) #1
declare i32 @pthread_join(i64, ptr) #1
declare i32 @sleep(i32) #1
declare i32 @nanosleep(ptr, ptr) #1
declare ptr @fopen(ptr, ptr) #1
declare i64 @fwrite(ptr, i64, i64, ptr) #1
declare i32 @fclose(ptr) #1
@A11 = constant float bitcast (i32 1065353216 to float)
@A10 = constant float bitcast (i32 0 to float)
@A22 = alias float, float* @A11
@Q21 = alias float, float* @A10
@Q12 = alias float, float* @A10
@A01 = constant float bitcast (i32 1008981770 to float)
@Q10 = alias float, float* @A10
@Q22 = constant float bitcast (i32 1036831949 to float)
@Q11 = alias float, float* @Q22
@A00 = alias float, float* @A11
@A02 = alias float, float* @A10
@Q01 = alias float, float* @A10
@Q02 = alias float, float* @A10
@A12 = alias float, float* @A01
@A21 = alias float, float* @A10
@Q20 = alias float, float* @A10
@A20 = alias float, float* @A10
@Q00 = alias float, float* @Q22

%StateChunk0 = type { float, float, float, float, float, float, float, float, float, float, float, float, i64, i64, i64 }
%State = type { float, float, float, float, float, float, float, float, float, float, float, float, i64, i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant <{ i64, i64, [1 x i8] }> <{
  i64 ptrtoint (i8* getelementptr inbounds (<{ i64, i64, [1 x i8] }>, <{ i64, i64, [1 x i8] }>* @str.0, i64 0, i32 2) to i64),
  i64 0,
  [1 x i8] c"\00"
}>, align 8
@str.1 = private unnamed_addr constant <{ i64, i64, [5 x i8] }> <{
  i64 ptrtoint (i8* getelementptr inbounds (<{ i64, i64, [5 x i8] }>, <{ i64, i64, [5 x i8] }>* @str.1, i64 0, i32 2) to i64),
  i64 4,
  [5 x i8] c"true\00"
}>, align 8
@str.2 = private unnamed_addr constant <{ i64, i64, [6 x i8] }> <{
  i64 ptrtoint (i8* getelementptr inbounds (<{ i64, i64, [6 x i8] }>, <{ i64, i64, [6 x i8] }>* @str.2, i64 0, i32 2) to i64),
  i64 5,
  [6 x i8] c"false\00"
}>, align 8
@str.3 = private unnamed_addr constant <{ i64, i64, [6 x i8] }> <{
  i64 ptrtoint (i8* getelementptr inbounds (<{ i64, i64, [6 x i8] }>, <{ i64, i64, [6 x i8] }>* @str.3, i64 0, i32 2) to i64),
  i64 5,
  [6 x i8] c"BOUND\00"
}>, align 8

@ll_empty_list = private unnamed_addr constant { i64, i64 } { i64 0, i64 0 }

define i64 @new_builder(ptr noalias nocapture align 8 %state) local_unnamed_addr #8 {
  entry:
  %sai1 = alloca i64, i64 1
%sp4 = getelementptr inbounds [1 x i8], [1 x i8]* @str.0, i64 0, i64 0
%t3 = ptrtoint i8* %sp4 to i64
  %sfp5 = getelementptr i64, ptr %sai1, i64 0
  store i64 %t3, i64* %sfp5, align 8
  %t0 = ptrtoint ptr %sai1 to i64
  ret i64 %t0
}

define i64 @with_capacity(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %sai1 = alloca i64, i64 1
%sp4 = getelementptr inbounds [1 x i8], [1 x i8]* @str.0, i64 0, i64 0
%t3 = ptrtoint i8* %sp4 to i64
  %sfp5 = getelementptr i64, ptr %sai1, i64 0
  store i64 %t3, i64* %sfp5, align 8
  %t0 = ptrtoint ptr %sai1 to i64
  ret i64 %t0
}

define i64 @append_char(ptr noalias nocapture align 8 %state, ptr %arg0, i32 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac1 = zext i32 %arg1 to i64
  %sai1 = alloca i64, i64 1
  %t4 = add i64 0, %ac0
  %fahp5 = inttoptr i64 %t4 to ptr
  %fafp6 = getelementptr i64, ptr %fahp5, i64 0
  %t3 = load i64, ptr %fafp6, align 8, !tbaa !1
  %t7 = add i64 0, %ac1
%t8 = add nsw i64 %t3, %t7
  %sfp9 = getelementptr i64, ptr %sai1, i64 0
  store i64 %t8, i64* %sfp9, align 8
  %t0 = ptrtoint ptr %sai1 to i64
  ret i64 %t0
}

define i64 @append_str(ptr noalias nocapture align 8 %state, ptr %arg0, ptr %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac1 = ptrtoint ptr %arg1 to i64
  %sai1 = alloca i64, i64 1
  %t4 = add i64 0, %ac0
  %fahp5 = inttoptr i64 %t4 to ptr
  %fafp6 = getelementptr i64, ptr %fahp5, i64 0
  %t3 = load i64, ptr %fafp6, align 8, !tbaa !1
  %t7 = add i64 0, %ac1
%cam8 = and i64 %t3, -4
%cbm9 = and i64 %t7, -4
%cha10 = inttoptr i64 %cam8 to ptr
%clp11 = getelementptr i64, ptr %cha10, i64 1
%cla12 = load i64, ptr %clp11, align 8
%chb13 = inttoptr i64 %cbm9 to ptr
%clq14 = getelementptr i64, ptr %chb13, i64 1
%clb15 = load i64, ptr %clq14, align 8
%ctl16 = add i64 %cla12, %clb15
%chs17 = add i64 16, %ctl16
%cas18 = add i64 %chs17, 1
%aam0 = call noalias ptr @malloc(i64 %cas18)
%chp19 = bitcast ptr %aam0 to ptr
%cba20 = ptrtoint ptr %aam0 to i64
%cdp21 = add i64 %cba20, 16
store i64 %cdp21, ptr %chp19, align 8
%cls22 = getelementptr i64, ptr %chp19, i64 1
store i64 %ctl16, ptr %cls22, align 8
%cad23 = load i64, ptr %cha10, align 8
%cac24 = inttoptr i64 %cad23 to ptr
%cds25 = getelementptr i8, ptr %aam0, i64 16
call void @llvm.memcpy.p0i8.p0i8.i64(i8* %cds25, ptr %cac24, i64 %cla12, i1 false)
%cdo26 = getelementptr i8, ptr %cds25, i64 %cla12
%cbd27 = load i64, ptr %chb13, align 8
%cbc28 = inttoptr i64 %cbd27 to ptr
call void @llvm.memcpy.p0i8.p0i8.i64(i8* %cdo26, ptr %cbc28, i64 %clb15, i1 false)
%cnt29 = getelementptr i8, ptr %cds25, i64 %ctl16
store i8 0, ptr %cnt29, align 1
%cta30 = and i64 %t3, 2
%cia31 = icmp ne i64 %cta30, 0
br i1 %cia31, label %free_a_32, label %af_a_32
free_a_32:
%cca32 = and i64 %t3, -4
%cfp33 = inttoptr i64 %cca32 to ptr
call void @free(ptr %cfp33)
br label %af_a_32
af_a_32:
%ctb34 = and i64 %t7, 2
%cib35 = icmp ne i64 %ctb34, 0
br i1 %cib35, label %free_b_36, label %af_b_36
free_b_36:
%ccb36 = and i64 %t7, -4
%cfq37 = inttoptr i64 %ccb36 to ptr
call void @free(ptr %cfq37)
br label %af_b_36
af_b_36:
%t38 = bitcast ptr %aam0 to ptr
%t39 = ptrtoint ptr %t38 to i64
%t40 = or i64 %t39, 2
  %sfp41 = getelementptr i64, ptr %sai1, i64 0
  store i64 %t40, i64* %sfp41, align 8
  %t0 = ptrtoint ptr %sai1 to i64
  ret i64 %t0
}

define i64 @append_int(ptr noalias nocapture align 8 %state, ptr %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %sai1 = alloca i64, i64 1
  %t4 = add i64 0, %ac0
  %fahp5 = inttoptr i64 %t4 to ptr
  %fafp6 = getelementptr i64, ptr %fahp5, i64 0
  %t3 = load i64, ptr %fafp6, align 8, !tbaa !1
  %t7 = add i64 0, %arg1
%t8 = add nsw i64 %t3, %t7
  %sfp9 = getelementptr i64, ptr %sai1, i64 0
  store i64 %t8, i64* %sfp9, align 8
  %t0 = ptrtoint ptr %sai1 to i64
  ret i64 %t0
}

define i64 @append_bool(ptr noalias nocapture align 8 %state, ptr %arg0, i8 %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ac1 = zext i8 %arg1 to i64
  %t0 = add i64 0, %ac1
  %gc1 = icmp ne i64 %t0, 0
  br i1 %gc1, label %g2_t, label %g2_e
  g2_t:
    %sai4 = alloca i64, i64 1
  %t7 = add i64 0, %ac0
  %fahp8 = inttoptr i64 %t7 to ptr
  %fafp9 = getelementptr i64, ptr %fahp8, i64 0
  %t6 = load i64, ptr %fafp9, align 8, !tbaa !1
%sp12 = getelementptr inbounds [5 x i8], [5 x i8]* @str.1, i64 0, i64 0
%t11 = ptrtoint i8* %sp12 to i64
%cam13 = and i64 %t6, -4
%cbm14 = and i64 %t11, -4
%cha15 = inttoptr i64 %cam13 to ptr
%clp16 = getelementptr i64, ptr %cha15, i64 1
%cla17 = load i64, ptr %clp16, align 8
%chb18 = inttoptr i64 %cbm14 to ptr
%clq19 = getelementptr i64, ptr %chb18, i64 1
%clb20 = load i64, ptr %clq19, align 8
%ctl21 = add i64 %cla17, %clb20
%chs22 = add i64 16, %ctl21
%cas23 = add i64 %chs22, 1
%aam1 = call noalias ptr @malloc(i64 %cas23)
%chp24 = bitcast ptr %aam1 to ptr
%cba25 = ptrtoint ptr %aam1 to i64
%cdp26 = add i64 %cba25, 16
store i64 %cdp26, ptr %chp24, align 8
%cls27 = getelementptr i64, ptr %chp24, i64 1
store i64 %ctl21, ptr %cls27, align 8
%cad28 = load i64, ptr %cha15, align 8
%cac29 = inttoptr i64 %cad28 to ptr
%cds30 = getelementptr i8, ptr %aam1, i64 16
call void @llvm.memcpy.p0i8.p0i8.i64(i8* %cds30, ptr %cac29, i64 %cla17, i1 false)
%cdo31 = getelementptr i8, ptr %cds30, i64 %cla17
%cbd32 = load i64, ptr %chb18, align 8
%cbc33 = inttoptr i64 %cbd32 to ptr
call void @llvm.memcpy.p0i8.p0i8.i64(i8* %cdo31, ptr %cbc33, i64 %clb20, i1 false)
%cnt34 = getelementptr i8, ptr %cds30, i64 %ctl21
store i8 0, ptr %cnt34, align 1
%cta35 = and i64 %t6, 2
%cia36 = icmp ne i64 %cta35, 0
br i1 %cia36, label %free_a_37, label %af_a_37
free_a_37:
%cca37 = and i64 %t6, -4
%cfp38 = inttoptr i64 %cca37 to ptr
call void @free(ptr %cfp38)
br label %af_a_37
af_a_37:
%ctb39 = and i64 %t11, 2
%cib40 = icmp ne i64 %ctb39, 0
br i1 %cib40, label %free_b_41, label %af_b_41
free_b_41:
%ccb41 = and i64 %t11, -4
%cfq42 = inttoptr i64 %ccb41 to ptr
call void @free(ptr %cfq42)
br label %af_b_41
af_b_41:
%t43 = bitcast ptr %aam1 to ptr
%t44 = ptrtoint ptr %t43 to i64
%t45 = or i64 %t44, 2
    %sfp46 = getelementptr i64, ptr %sai4, i64 0
    store i64 %t45, i64* %sfp46, align 8
    %t3 = ptrtoint ptr %sai4 to i64
    ret i64 %t3
  g2_e:
  %sai48 = alloca i64, i64 1
  %t51 = add i64 0, %ac0
  %fahp52 = inttoptr i64 %t51 to ptr
  %fafp53 = getelementptr i64, ptr %fahp52, i64 0
  %t50 = load i64, ptr %fafp53, align 8, !tbaa !1
%sp56 = getelementptr inbounds [6 x i8], [6 x i8]* @str.2, i64 0, i64 0
%t55 = ptrtoint i8* %sp56 to i64
%cam57 = and i64 %t50, -4
%cbm58 = and i64 %t55, -4
%cha59 = inttoptr i64 %cam57 to ptr
%clp60 = getelementptr i64, ptr %cha59, i64 1
%cla61 = load i64, ptr %clp60, align 8
%chb62 = inttoptr i64 %cbm58 to ptr
%clq63 = getelementptr i64, ptr %chb62, i64 1
%clb64 = load i64, ptr %clq63, align 8
%ctl65 = add i64 %cla61, %clb64
%chs66 = add i64 16, %ctl65
%cas67 = add i64 %chs66, 1
%aam2 = call noalias ptr @malloc(i64 %cas67)
%chp68 = bitcast ptr %aam2 to ptr
%cba69 = ptrtoint ptr %aam2 to i64
%cdp70 = add i64 %cba69, 16
store i64 %cdp70, ptr %chp68, align 8
%cls71 = getelementptr i64, ptr %chp68, i64 1
store i64 %ctl65, ptr %cls71, align 8
%cad72 = load i64, ptr %cha59, align 8
%cac73 = inttoptr i64 %cad72 to ptr
%cds74 = getelementptr i8, ptr %aam2, i64 16
call void @llvm.memcpy.p0i8.p0i8.i64(i8* %cds74, ptr %cac73, i64 %cla61, i1 false)
%cdo75 = getelementptr i8, ptr %cds74, i64 %cla61
%cbd76 = load i64, ptr %chb62, align 8
%cbc77 = inttoptr i64 %cbd76 to ptr
call void @llvm.memcpy.p0i8.p0i8.i64(i8* %cdo75, ptr %cbc77, i64 %clb64, i1 false)
%cnt78 = getelementptr i8, ptr %cds74, i64 %ctl65
store i8 0, ptr %cnt78, align 1
%cta79 = and i64 %t50, 2
%cia80 = icmp ne i64 %cta79, 0
br i1 %cia80, label %free_a_81, label %af_a_81
free_a_81:
%cca81 = and i64 %t50, -4
%cfp82 = inttoptr i64 %cca81 to ptr
call void @free(ptr %cfp82)
br label %af_a_81
af_a_81:
%ctb83 = and i64 %t55, 2
%cib84 = icmp ne i64 %ctb83, 0
br i1 %cib84, label %free_b_85, label %af_b_85
free_b_85:
%ccb85 = and i64 %t55, -4
%cfq86 = inttoptr i64 %ccb85 to ptr
call void @free(ptr %cfq86)
br label %af_b_85
af_b_85:
%t87 = bitcast ptr %aam2 to ptr
%t88 = ptrtoint ptr %t87 to i64
%t89 = or i64 %t88, 2
  %sfp90 = getelementptr i64, ptr %sai48, i64 0
  store i64 %t89, i64* %sfp90, align 8
  %t47 = ptrtoint ptr %sai48 to i64
  ret i64 %t47
}

define i64 @append_float(ptr noalias nocapture align 8 %state, ptr %arg0, float %arg1) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %ai91 = bitcast float %arg1 to i32
  %ac1 = zext i32 %ai91 to i64
  %sai1 = alloca i64, i64 1
  %t4 = add i64 0, %ac0
  %fahp5 = inttoptr i64 %t4 to ptr
  %fafp6 = getelementptr i64, ptr %fahp5, i64 0
  %t3 = load i64, ptr %fafp6, align 8, !tbaa !1
%t8 = add nsw i64 %t3, %ac1
  %sfp9 = getelementptr i64, ptr %sai1, i64 0
  store i64 %t8, i64* %sfp9, align 8
  %t0 = ptrtoint ptr %sai1 to i64
  ret i64 %t0
}

define i64 @to_string(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t1 = add i64 0, %ac0
  %fahp2 = inttoptr i64 %t1 to ptr
  %fafp3 = getelementptr i64, ptr %fahp2, i64 0
  %t0 = load i64, ptr %fafp3, align 8, !tbaa !1
  ret i64 %t0
}

define i64 @clear(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %sai1 = alloca i64, i64 1
%sp4 = getelementptr inbounds [1 x i8], [1 x i8]* @str.0, i64 0, i64 0
%t3 = ptrtoint i8* %sp4 to i64
  %sfp5 = getelementptr i64, ptr %sai1, i64 0
  store i64 %t3, i64* %sfp5, align 8
  %t0 = ptrtoint ptr %sai1 to i64
  ret i64 %t0
}

define i64 @len(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = add i64 0, %ac0
  %fahp3 = inttoptr i64 %t2 to ptr
  %fafp4 = getelementptr i64, ptr %fahp3, i64 0
  %t1 = load i64, ptr %fafp4, align 8, !tbaa !1
  %php5 = inttoptr i64 %t1 to ptr
  %plp6 = getelementptr i64, ptr %php5, i64 1
  %t0 = load i64, ptr %plp6, align 8, !tbaa !1
  ret i64 %t0
}

define i64 @is_empty(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t3 = add i64 0, %ac0
  %fahp4 = inttoptr i64 %t3 to ptr
  %fafp5 = getelementptr i64, ptr %fahp4, i64 0
  %t2 = load i64, ptr %fafp5, align 8, !tbaa !1
  %php6 = inttoptr i64 %t2 to ptr
  %plp7 = getelementptr i64, ptr %php6, i64 1
  %t1 = load i64, ptr %plp7, align 8, !tbaa !1
%t9 = add i64 0, 0
%c10 = icmp eq i64 %t1, %t9
  %rz11 = zext i1 %c10 to i64
  ret i64 %rz11
}

define i64 @capacity(ptr noalias nocapture align 8 %state, ptr %arg0) local_unnamed_addr #8 {
  entry:
  %ac0 = ptrtoint ptr %arg0 to i64
  %t2 = add i64 0, %ac0
  %fahp3 = inttoptr i64 %t2 to ptr
  %fafp4 = getelementptr i64, ptr %fahp3, i64 0
  %t1 = load i64, ptr %fafp4, align 8, !tbaa !1
  %php5 = inttoptr i64 %t1 to ptr
  %plp6 = getelementptr i64, ptr %php5, i64 1
  %t0 = load i64, ptr %plp6, align 8, !tbaa !1
  ret i64 %t0
}

define i64 @read_i64(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %t2 = add i64 0, %arg1
  %xhp3 = inttoptr i64 %arg0 to ptr
  %xdp4 = load i64, ptr %xhp3, align 8
  %xde5 = inttoptr i64 %xdp4 to ptr
  %xep6 = getelementptr i64, ptr %xde5, i64 %t2
  %t0 = load i64, ptr %xep6, align 8
  ret i64 %t0
}

define void @write_i64(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  %t0 = add i64 0, %arg2
  %t1 = add i64 0, %arg1
  %lhp2 = inttoptr i64 %arg0 to ptr
  %ldp3 = load i64, ptr %lhp2, align 8
  %lde4 = inttoptr i64 %ldp3 to ptr
  %lep5 = getelementptr i64, ptr %lde4, i64 %t1
  store i64 %t0, ptr %lep5, align 8
  ret void
}

define i64 @address(ptr noalias nocapture align 8 %state, i64 %arg0) local_unnamed_addr #8 {
  entry:
  %t0 = add i64 0, %arg0 ; ptr
  ret i64 %t0
}

define i64 @read_byte(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1) local_unnamed_addr #8 {
  entry:
  %t1 = add i64 0, %arg1
%t3 = add i64 0, 8
%t4 = sdiv i64 %t1, %t3
  ; let elem_idx = %t4
  %t7 = add i64 0, %arg1
%t9 = add i64 0, 8
%t10 = srem i64 %t7, %t9
%t12 = add i64 0, 8
%t13 = mul nsw i64 %t10, %t12
  ; let byte_shift = %t13
  %t16 = add i64 0, %t4
  %xhp17 = inttoptr i64 %arg0 to ptr
  %xdp18 = load i64, ptr %xhp17, align 8
  %xde19 = inttoptr i64 %xdp18 to ptr
  %xep20 = getelementptr i64, ptr %xde19, i64 %t16
  %t14 = load i64, ptr %xep20, align 8
  ; let word = %t14
  %t23 = add i64 0, %t14
  %t24 = add i64 0, %t13
%t25 = lshr i64 %t23, %t24
%t27 = add i64 0, 255
%t28 = and i64 %t25, %t27
  ret i64 %t28
}

define void @copy(ptr noalias nocapture align 8 %state, i64 %arg0, i64 %arg1, i64 %arg2) local_unnamed_addr #8 {
  entry:
  ret void
}

define internal i64 @file_open(i64 %path, i64 %flags, i64 %mode) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_open (i64 %path, i64 %flags, i64 %mode);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_close(i64 %fd) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_close (i64 %fd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_read(i64 %fd, i64 %buf, i64 %count) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_read (i64 %fd, i64 %buf, i64 %count);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_write(i64 %fd, i64 %buf, i64 %count) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_write (i64 %fd, i64 %buf, i64 %count);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_lseek(i64 %fd, i64 %offset, i64 %whence) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_lseek (i64 %fd, i64 %offset, i64 %whence);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_pread(i64 %fd, i64 %buf, i64 %count, i64 %offset) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_pread (i64 %fd, i64 %buf, i64 %count, i64 %offset);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_pwrite(i64 %fd, i64 %buf, i64 %count, i64 %offset) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_pwrite (i64 %fd, i64 %buf, i64 %count, i64 %offset);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_stat(i64 %path, i64 %buf) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_stat (i64 %path, i64 %buf);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_fstat(i64 %fd, i64 %buf) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_fstat (i64 %fd, i64 %buf);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_ftruncate(i64 %fd, i64 %length) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_ftruncate (i64 %fd, i64 %length);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_fsync(i64 %fd) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_fsync (i64 %fd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_dup(i64 %fd) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_dup (i64 %fd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_dup2(i64 %oldfd, i64 %newfd) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_dup2 (i64 %oldfd, i64 %newfd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_fcntl(i64 %fd, i64 %cmd, i64 %arg) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_fcntl (i64 %fd, i64 %cmd, i64 %arg);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_socket(i64 %domain, i64 %type_, i64 %protocol) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_socket (i64 %domain, i64 %type_, i64 %protocol);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_bind(i64 %fd, i64 %addr, i64 %addrlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_bind (i64 %fd, i64 %addr, i64 %addrlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_listen(i64 %fd, i64 %backlog) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_listen (i64 %fd, i64 %backlog);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_accept(i64 %fd, i64 %addr, i64 %addrlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_accept (i64 %fd, i64 %addr, i64 %addrlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_connect(i64 %fd, i64 %addr, i64 %addrlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_connect (i64 %fd, i64 %addr, i64 %addrlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_send(i64 %fd, i64 %buf, i64 %len, i64 %flags) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_send (i64 %fd, i64 %buf, i64 %len, i64 %flags);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_recv(i64 %fd, i64 %buf, i64 %len, i64 %flags) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_recv (i64 %fd, i64 %buf, i64 %len, i64 %flags);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_sendto(i64 %fd, i64 %buf, i64 %len, i64 %flags, i64 %dest, i64 %destlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_sendto (i64 %fd, i64 %buf, i64 %len, i64 %flags, i64 %dest, i64 %destlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_recvfrom(i64 %fd, i64 %buf, i64 %len, i64 %flags, i64 %src, i64 %srclen) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_recvfrom (i64 %fd, i64 %buf, i64 %len, i64 %flags, i64 %src, i64 %srclen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_setsockopt(i64 %fd, i64 %level, i64 %optname, i64 %optval, i64 %optlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_setsockopt (i64 %fd, i64 %level, i64 %optname, i64 %optval, i64 %optlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getsockopt(i64 %fd, i64 %level, i64 %optname, i64 %optval, i64 %optlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_getsockopt (i64 %fd, i64 %level, i64 %optname, i64 %optval, i64 %optlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_shutdown(i64 %fd, i64 %how) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_shutdown (i64 %fd, i64 %how);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_sigaction(i64 %sig_num, i64 %action, i64 %old_action) local_unnamed_addr #0 {
  entry:
  %r = call i64@__sigaction__ (i64 %sig_num, i64 %action, i64 %old_action);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_sigprocmask(i64 %how, i64 %set, i64 %old_set) local_unnamed_addr #0 {
  entry:
  %r = call i64@__sigprocmask__ (i64 %how, i64 %set, i64 %old_set);
  ret i64 %r;
  ret i64 0
}

define internal i64 @pipe(i64 %pipefd) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_pipe (i64 %pipefd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @shm_open(i64 %name, i64 %oflag, i64 %mode) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_shm_open (i64 %name, i64 %oflag, i64 %mode);
  ret i64 %r;
  ret i64 0
}

define internal i64 @shm_unlink(i64 %name) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_shm_unlink (i64 %name);
  ret i64 %r;
  ret i64 0
}

define internal i64 @sem_open(i64 %name, i64 %oflag, i64 %mode, i64 %value) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_sem_open (i64 %name, i64 %oflag, i64 %mode, i64 %value);
  ret i64 %r;
  ret i64 0
}

define internal i64 @sem_wait(i64 %sem) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_sem_wait (i64 %sem);
  ret i64 %r;
  ret i64 0
}

define internal i64 @sem_post(i64 %sem) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_sem_post (i64 %sem);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_thread_create(i64 %fn_ptr, i64 %arg) local_unnamed_addr #0 {
  entry:
  %r = call i64@__thread_create__ (i64 %fn_ptr, i64 %arg);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_thread_join(i64 %thread) local_unnamed_addr #0 {
  entry:
  %r = call i64@__thread_join__ (i64 %thread);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_thread_exit(i64 %code) local_unnamed_addr #0 {
  entry:
  %r = call i64@__thread_exit__ (i64 %code);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_mutex_lock(i64 %mptr) local_unnamed_addr #0 {
  entry:
  %r = call i64@__mutex_lock__ (i64 %mptr);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_mutex_unlock(i64 %mptr) local_unnamed_addr #0 {
  entry:
  %r = call i64@__mutex_unlock__ (i64 %mptr);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_condvar_wait(i64 %cptr, i64 %mptr) local_unnamed_addr #0 {
  entry:
  %r = call i64@__condvar_wait__ (i64 %cptr, i64 %mptr);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_condvar_signal(i64 %cptr) local_unnamed_addr #0 {
  entry:
  %r = call i64@__condvar_signal__ (i64 %cptr);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_condvar_broadcast(i64 %cptr) local_unnamed_addr #0 {
  entry:
  %r = call i64@__condvar_broadcast__ (i64 %cptr);
  ret i64 %r;
  ret i64 0
}

define internal i64 @mkdir(i64 %path, i64 %mode) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_mkdir (i64 %path, i64 %mode);
  ret i64 %r;
  ret i64 0
}

define internal i64 @rmdir(i64 %path) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_rmdir (i64 %path);
  ret i64 %r;
  ret i64 0
}

define internal i64 @unlink(i64 %path) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_unlink (i64 %path);
  ret i64 %r;
  ret i64 0
}

define internal i64 @rename(i64 %oldpath, i64 %newpath) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_rename (i64 %oldpath, i64 %newpath);
  ret i64 %r;
  ret i64 0
}

define internal i64 @symlink(i64 %target, i64 %linkpath) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_symlink (i64 %target, i64 %linkpath);
  ret i64 %r;
  ret i64 0
}

define internal i64 @read_link(i64 %path, i64 %buf, i64 %bufsiz) local_unnamed_addr #0 {
  entry:
  %r = call i64@__readlink__ (i64 %path, i64 %buf, i64 %bufsiz);
  ret i64 %r;
  ret i64 0
}

define internal i64 @link_path(i64 %oldpath, i64 %newpath) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_link (i64 %oldpath, i64 %newpath);
  ret i64 %r;
  ret i64 0
}

define internal i64 @getcwd(i64 %buf, i64 %size) local_unnamed_addr #0 {
  entry:
  %r = call i64@__getcwd__ (i64 %buf, i64 %size);
  ret i64 %r;
  ret i64 0
}

define internal i64 @chdir(i64 %path) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_chdir (i64 %path);
  ret i64 %r;
  ret i64 0
}

define internal i64 @readdir(i64 %dirp, i64 %buf) local_unnamed_addr #0 {
  entry:
  %r = call i64@__readdir__ (i64 %dirp, i64 %buf);
  ret i64 %r;
  ret i64 0
}

define internal i64 @chmod(i64 %path, i64 %mode) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_chmod (i64 %path, i64 %mode);
  ret i64 %r;
  ret i64 0
}

define internal i64 @chown(i64 %path, i64 %owner, i64 %group) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_chown (i64 %path, i64 %owner, i64 %group);
  ret i64 %r;
  ret i64 0
}

define internal i64 @umask(i64 %mask) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_umask (i64 %mask);
  ret i64 %r;
  ret i64 0
}

define internal i64 @access(i64 %path, i64 %mode) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_access (i64 %path, i64 %mode);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getpid() local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_getpid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getppid() local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_getppid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_tty_raw_mode(i64 %enable) local_unnamed_addr #0 {
  entry:
  %r = call i64@__tty_raw_mode__ (i64 %enable);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_tty_size() local_unnamed_addr #0 {
  entry:
  %r = call i64@__tty_size__ ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_tty_read_key(i64 %fd) local_unnamed_addr #0 {
  entry:
  %r = call i64@__tty_read_key__ (i64 %fd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_ioctl(i64 %fd, i64 %request, i64 %argp) local_unnamed_addr #0 {
  entry:
  %r = call i64@__ioctl__ (i64 %fd, i64 %request, i64 %argp);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_isatty(i64 %fd) local_unnamed_addr #0 {
  entry:
  %r = call i64@__isatty__ (i64 %fd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getuid() local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_getuid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_geteuid() local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_geteuid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getgid() local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_getgid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getegid() local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_getegid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_clock_gettime(i64 %clock_id, i64 %tp) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_clock_gettime (i64 %clock_id, i64 %tp);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_nanosleep(i64 %req, i64 %rem) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_nanosleep (i64 %req, i64 %rem);
  ret i64 %r;
  ret i64 0
}

define internal i64 @mmap(i64 %addr, i64 %length, i64 %prot, i64 %flags, i64 %fd, i64 %offset) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_mmap (i64 %addr, i64 %length, i64 %prot, i64 %flags, i64 %fd, i64 %offset);
  ret i64 %r;
  ret i64 0
}

define internal i64 @munmap(i64 %addr, i64 %length) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_munmap (i64 %addr, i64 %length);
  ret i64 %r;
  ret i64 0
}

define internal i64 @mprotect(i64 %addr, i64 %length, i64 %prot) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_mprotect (i64 %addr, i64 %length, i64 %prot);
  ret i64 %r;
  ret i64 0
}

define internal i64 @brk(i64 %addr) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_brk (i64 %addr);
  ret i64 %r;
  ret i64 0
}

define internal i64 @mlock(i64 %addr, i64 %length) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_mlock (i64 %addr, i64 %length);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getrandom(i64 %buf, i64 %len, i64 %flags) local_unnamed_addr #0 {
  entry:
  %r = call i64@__getrandom__ (i64 %buf, i64 %len, i64 %flags);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_sched_yield() local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_sched_yield ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getrlimit(i64 %resource) local_unnamed_addr #0 {
  entry:
  %r = call i64@__getrlimit__ (i64 %resource);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_setrlimit(i64 %resource, i64 %packed) local_unnamed_addr #0 {
  entry:
  %r = call i64@__setrlimit__ (i64 %resource, i64 %packed);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_uname() local_unnamed_addr #0 {
  entry:
  %r = call i64@__uname__ ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_hostname() local_unnamed_addr #0 {
  entry:
  %r = call i64@__hostname__ ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_pagesize() local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_pagesize ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_cpu_count() local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_cpu_count ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_mkstemp(i64 %tmpl) local_unnamed_addr #0 {
  entry:
  %r = call i64@__mkstemp__ (i64 %tmpl);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_mkdtemp(i64 %tmpl) local_unnamed_addr #0 {
  entry:
  %r = call i64@__mkdtemp__ (i64 %tmpl);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_dlopen(i64 %filename) local_unnamed_addr #0 {
  entry:
  %r = call i64@__dlopen__ (i64 %filename);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_dlsym(i64 %handle, i64 %symbol) local_unnamed_addr #0 {
  entry:
  %r = call i64@__dlsym__ (i64 %handle, i64 %symbol);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_dlclose(i64 %handle) local_unnamed_addr #0 {
  entry:
  %r = call i64@__dlclose__ (i64 %handle);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_backtrace() local_unnamed_addr #0 {
  entry:
  %r = call i64@__backtrace__ ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_ring_push(i64 %handle, i64 %val) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_ring_push (i64 %handle, i64 %val);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_ring_pop(i64 %handle) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_ring_pop (i64 %handle);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_print(i64 %s) local_unnamed_addr #0 {
  entry:
  %r = call i64@__print (i64 %s);
  ret i64 %r;
  ret i64 0
}

define internal i64 @atomic_load(i64 %ptr) local_unnamed_addr #0 {
  entry:
  %p = inttoptr i64 %ptr to ptr;
  %r = load atomic i64, ptr %p seq_cst, align 8;
  ret i64 %r;
  ret i64 0
}

define internal i64 @atomic_store(i64 %ptr, i64 %val) local_unnamed_addr #0 {
  entry:
  %p = inttoptr i64 %ptr to ptr;
  store atomic i64 %val, ptr %p seq_cst, align 8;
  %r = add i64 0, 1;
  ret i64 %r;
  ret i64 0
}

define internal i64 @atomic_cas(i64 %ptr, i64 %expected, i64 %desired) local_unnamed_addr #0 {
  entry:
  %p = inttoptr i64 %ptr to ptr;
  %pair = cmpxchg ptr %p, i64 %expected, i64 %desired seq_cst seq_cst;
  %r = extractvalue {i64, i1 }%pair, 0;
  ret i64 %r;
  ret i64 0
}

define internal i64 @atomic_xchg(i64 %ptr, i64 %val) local_unnamed_addr #0 {
  entry:
  %p = inttoptr i64 %ptr to ptr;
  %r = atomicrmw xchg ptr %p, i64 %val seq_cst, align 8;
  ret i64 %r;
  ret i64 0
}

define internal i64 @atomic_add(i64 %ptr, i64 %val) local_unnamed_addr #0 {
  entry:
  %p = inttoptr i64 %ptr to ptr;
  %r = atomicrmw add ptr %p, i64 %val seq_cst, align 8;
  ret i64 %r;
  ret i64 0
}

define internal i64 @fence() local_unnamed_addr #0 {
  entry:
  fence seq_cst;
  %r = add i64 0, 1;
  ret i64 %r;
  ret i64 0
}

define internal i64 @futex(i64 %uaddr, i64 %opcode, i64 %val, i64 %timeout, i64 %uaddr2, i64 %val3) local_unnamed_addr #0 {
  entry:
  %r = call i64@briev_futex (i64 %uaddr, i64 %opcode, i64 %val, i64 %timeout, i64 %uaddr2, i64 %val3);
  ret i64 %r;
  ret i64 0
}

define void @tick(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp2 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t1 = load i64, i64* %fdp2, align 8
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t3 = load i64, i64* %fdp4, align 8
%c5 = icmp slt i64 %t1, %t3
  %pi6 = and i1 %c5, true
  br i1 %pi6, label %ps8, label %pp7
  pp7:
    unreachable
  ps8:
  call void @llvm.assume(i1 %pi6)
  %il13 = load float, float* @A00, align 4
  %fdp15 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t14 = load float, ptr %fdp15, align 4
%t16 = fmul fast float %il13, %t14
  %il19 = load float, float* @A01, align 4
  %fdp21 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t20 = load float, ptr %fdp21, align 4
%t22 = fmul fast float %il19, %t20
%t23 = fadd fast float %t16, %t22
  %il26 = load float, float* @A02, align 4
  %fdp28 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t27 = load float, ptr %fdp28, align 4
%t29 = fmul fast float %il26, %t27
%t30 = fadd fast float %t23, %t29
  %ap_31 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store float %t30, ptr %ap_31, align 4, !tbaa !3
  %fdp34 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t33 = load float, ptr %fdp34, align 4
  %il36 = load float, float* @Q00, align 4
%t37 = fadd fast float %t33, %il36
  %ap_38 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store float %t37, ptr %ap_38, align 4, !tbaa !3
  %fdp41 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t40 = load float, ptr %fdp41, align 4
  %il43 = load float, float* @Q01, align 4
%t44 = fadd fast float %t40, %il43
  %ap_45 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store float %t44, ptr %ap_45, align 4, !tbaa !3
  %fdp48 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t47 = load float, ptr %fdp48, align 4
  %il50 = load float, float* @Q02, align 4
%t51 = fadd fast float %t47, %il50
  %ap_52 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store float %t51, ptr %ap_52, align 4, !tbaa !3
  %fdp55 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t54 = load float, ptr %fdp55, align 4
  %il57 = load float, float* @Q10, align 4
%t58 = fadd fast float %t54, %il57
  %ap_59 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store float %t58, ptr %ap_59, align 4, !tbaa !3
  %fdp62 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t61 = load float, ptr %fdp62, align 4
  %il64 = load float, float* @Q11, align 4
%t65 = fadd fast float %t61, %il64
  %ap_66 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store float %t65, ptr %ap_66, align 4, !tbaa !3
  %fdp69 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t68 = load float, ptr %fdp69, align 4
  %il71 = load float, float* @Q12, align 4
%t72 = fadd fast float %t68, %il71
  %ap_73 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store float %t72, ptr %ap_73, align 4, !tbaa !3
  %fdp76 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t75 = load float, ptr %fdp76, align 4
  %il78 = load float, float* @Q20, align 4
%t79 = fadd fast float %t75, %il78
  %ap_80 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store float %t79, ptr %ap_80, align 4, !tbaa !3
  %fdp83 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t82 = load float, ptr %fdp83, align 4
  %il85 = load float, float* @Q21, align 4
%t86 = fadd fast float %t82, %il85
  %ap_87 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store float %t86, ptr %ap_87, align 4, !tbaa !3
  %fdp90 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t89 = load float, ptr %fdp90, align 4
  %il92 = load float, float* @Q22, align 4
%t93 = fadd fast float %t89, %il92
  %ap_94 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  store float %t93, ptr %ap_94, align 4, !tbaa !3
  %fdp97 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t96 = load i64, i64* %fdp97, align 8
%t99 = add i64 0, 1
%t100 = add nsw i64 %t96, %t99
  %ap_101 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store i64 %t100, ptr %ap_101, align 8, !tbaa !1
  %il106 = load float, float* @A10, align 4
  %fdp108 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t107 = load float, ptr %fdp108, align 4
%t109 = fmul fast float %il106, %t107
  %il112 = load float, float* @A11, align 4
  %fdp114 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t113 = load float, ptr %fdp114, align 4
%t115 = fmul fast float %il112, %t113
%t116 = fadd fast float %t109, %t115
  %il119 = load float, float* @A12, align 4
  %fdp121 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t120 = load float, ptr %fdp121, align 4
%t122 = fmul fast float %il119, %t120
%t123 = fadd fast float %t116, %t122
  %ap_124 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store float %t123, ptr %ap_124, align 4, !tbaa !3
  %il129 = load float, float* @A20, align 4
  %fdp131 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t130 = load float, ptr %fdp131, align 4
%t132 = fmul fast float %il129, %t130
  %il135 = load float, float* @A21, align 4
  %fdp137 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t136 = load float, ptr %fdp137, align 4
%t138 = fmul fast float %il135, %t136
%t139 = fadd fast float %t132, %t138
  %il142 = load float, float* @A22, align 4
  %fdp144 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t143 = load float, ptr %fdp144, align 4
%t145 = fmul fast float %il142, %t143
%t146 = fadd fast float %t139, %t145
  %ap_147 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store float %t146, ptr %ap_147, align 4, !tbaa !3
  %fdp151 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t150 = load i64, i64* %fdp151, align 8
%t153 = add i64 0, 5000000
%t154 = srem i64 %t150, %t153
%t156 = add i64 0, 0
%c157 = icmp eq i64 %t154, %t156
  br i1 %c157, label %g158_t, label %g158_e
  g158_t:
  %fdp168 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t167 = load float, ptr %fdp168, align 4
  %fdp170 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t169 = load float, ptr %fdp170, align 4
%t171 = fadd fast float %t167, %t169
  %fdp173 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t172 = load float, ptr %fdp173, align 4
%t174 = fadd fast float %t171, %t172
  %fdp176 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t175 = load float, ptr %fdp176, align 4
%t177 = fadd fast float %t174, %t175
  %fdp179 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t178 = load float, ptr %fdp179, align 4
%t180 = fadd fast float %t177, %t178
  %fdp182 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t181 = load float, ptr %fdp182, align 4
%t183 = fadd fast float %t180, %t181
  %fdp185 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t184 = load float, ptr %fdp185, align 4
%t186 = fadd fast float %t183, %t184
  %fdp188 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t187 = load float, ptr %fdp188, align 4
%t189 = fadd fast float %t186, %t187
  %fdp191 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t190 = load float, ptr %fdp191, align 4
%t192 = fadd fast float %t189, %t190
    ; let trace = %t192
  %fdp198 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t197 = load float, ptr %fdp198, align 4
  %fdp200 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t199 = load float, ptr %fdp200, align 4
%t201 = fadd fast float %t197, %t199
  %fdp203 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t202 = load float, ptr %fdp203, align 4
%t204 = fadd fast float %t201, %t202
%t206 = fadd fast float %t204, %t192
    %pfd207 = fpext float %t206 to double
    %pso208 = load volatile ptr, ptr @stdout
    %pff209 = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0
    %ppf210 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso208, ptr %pff209, double %pfd207)
    %t193 = zext i32 %ppf210 to i64
    br label %g158_tx
  g158_tx:
    br label %g158_e
  g158_e:
  ret void
}

define internal i1 @pre_tick(ptr noalias nocapture align 8 %state) #10 {
  entry:
  %fdp2 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t1 = load i64, i64* %fdp2, align 8
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t3 = load i64, i64* %fdp4, align 8
%c5 = icmp slt i64 %t1, %t3
  ret i1 %c5
}
define void @init_state(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 {
  entry:
  %ip_0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %ip_0b = bitcast i32 0 to float
  store float %ip_0b, ptr %ip_0, align 4
  %ip_1 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %ip_1b = bitcast i32 0 to float
  store float %ip_1b, ptr %ip_1, align 4
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
  %ip_5b = bitcast i32 0 to float
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
  %ip_9b = bitcast i32 0 to float
  store float %ip_9b, ptr %ip_9, align 4
  %ip_10 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %ip_10b = bitcast i32 0 to float
  store float %ip_10b, ptr %ip_10, align 4
  %ip_11 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %ip_11b = bitcast i32 0 to float
  store float %ip_11b, ptr %ip_11, align 4
  %ip_12 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
%t1 = add i64 0, 0
  store i64 %t1, ptr %ip_12, align 8
  %ip_13 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
%sp5 = getelementptr inbounds [6 x i8], [6 x i8]* @str.3, i64 0, i64 0
%t4 = ptrtoint i8* %sp5 to i64
  %gsr6 = inttoptr i64 %t4 to ptr
  %gsp7 = bitcast ptr %gsr6 to ptr
  %gdp8 = load i64, ptr %gsp7, align 8
  %gnp9 = inttoptr i64 %gdp8 to ptr
  %gnv10 = call ptr @getenv(ptr %gnp9)
  %gnvl11 = icmp eq ptr %gnv10, null
  br i1 %gnvl11, label %genv_nul12, label %genv_ok13
  genv_nul12:
    br label %genv_af14
  genv_ok13:
  %gav15 = call i64 @atol(ptr %gnv10)
    br label %genv_af14
  genv_af14:
  %t2 = phi i64 [ 0, %genv_nul12 ], [ %gav15, %genv_ok13 ]
  store i64 %t2, ptr %ip_13, align 8
  %ip_14 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  store i64 0, ptr %ip_14, align 8
  ret void
}


define i32 @main() local_unnamed_addr #9 {
  entry:
  %state_0 = alloca %StateChunk0, align 8
  %state = alloca %State, align 8
  %ip_16 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %ip_0b = bitcast i32 0 to float
  store float %ip_0b, ptr %ip_16, align 4
  %ip_17 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
  %ip_1b = bitcast i32 0 to float
  store float %ip_1b, ptr %ip_17, align 4
  %ip_18 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %ip_2b = bitcast i32 0 to float
  store float %ip_2b, ptr %ip_18, align 4
  %ip_19 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %ip_3b = bitcast i32 0 to float
  store float %ip_3b, ptr %ip_19, align 4
  %ip_20 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %ip_4b = bitcast i32 0 to float
  store float %ip_4b, ptr %ip_20, align 4
  %ip_21 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %ip_5b = bitcast i32 0 to float
  store float %ip_5b, ptr %ip_21, align 4
  %ip_22 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %ip_6b = bitcast i32 0 to float
  store float %ip_6b, ptr %ip_22, align 4
  %ip_23 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
  %ip_7b = bitcast i32 0 to float
  store float %ip_7b, ptr %ip_23, align 4
  %ip_24 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
  %ip_8b = bitcast i32 0 to float
  store float %ip_8b, ptr %ip_24, align 4
  %ip_25 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
  %ip_9b = bitcast i32 0 to float
  store float %ip_9b, ptr %ip_25, align 4
  %ip_26 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
  %ip_10b = bitcast i32 0 to float
  store float %ip_10b, ptr %ip_26, align 4
  %ip_27 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
  %ip_11b = bitcast i32 0 to float
  store float %ip_11b, ptr %ip_27, align 4
  %ip_28 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
%t30 = add i64 0, 0
  store i64 %t30, ptr %ip_28, align 8
  %ip_31 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
%sp35 = getelementptr inbounds [6 x i8], [6 x i8]* @str.3, i64 0, i64 0
%t34 = ptrtoint i8* %sp35 to i64
  %gsr36 = inttoptr i64 %t34 to ptr
  %gsp37 = bitcast ptr %gsr36 to ptr
  %gdp38 = load i64, ptr %gsp37, align 8
  %gnp39 = inttoptr i64 %gdp38 to ptr
  %gnv40 = call ptr @getenv(ptr %gnp39)
  %gnvl41 = icmp eq ptr %gnv40, null
  br i1 %gnvl41, label %genv_nul42, label %genv_ok43
  genv_nul42:
    br label %genv_af44
  genv_ok43:
  %gav45 = call i64 @atol(ptr %gnv40)
    br label %genv_af44
  genv_af44:
  %t32 = phi i64 [ 0, %genv_nul42 ], [ %gav45, %genv_ok43 ]
  store i64 %t32, ptr %ip_31, align 8
  %ip_46 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 14
  store i64 0, ptr %ip_46, align 8
  %arptr3 = alloca i8*, align 8
  %arend3 = alloca i8*, align 8
  %arbase3 = alloca i8*, align 8
  %arinit3 = call ptr @malloc(i64 65536)
  store i8* %arinit3, ptr %arptr3, align 8
  store i8* %arinit3, ptr %arbase3, align 8
  %arieu3 = getelementptr i8, ptr %arinit3, i64 65536
  store i8* %arieu3, ptr %arend3, align 8
  %gt_47 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
  %cnt_bound_16 = load i64, ptr %gt_47, align 8
  br label %pre_phi
pre_phi:
  %init_cnt_48 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 14
  %init_cycle_count_49 = load i64, ptr %init_cnt_48, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_50 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %init_p00_51 = load float, ptr %init_cnt_50, align 4, !tbaa !3
  %init_cnt_52 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %init_p01_53 = load float, ptr %init_cnt_52, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_54 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %init_p02_55 = load float, ptr %init_cnt_54, align 4, !tbaa !3
  %init_cnt_56 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %init_p10_57 = load float, ptr %init_cnt_56, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_58 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
  %init_p11_59 = load float, ptr %init_cnt_58, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_60 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
  %init_p12_61 = load float, ptr %init_cnt_60, align 4, !tbaa !3
  %init_cnt_62 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
  %init_p20_63 = load float, ptr %init_cnt_62, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_64 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
  %init_p21_65 = load float, ptr %init_cnt_64, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_66 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
  %init_p22_67 = load float, ptr %init_cnt_66, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_68 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
  %init_total_69 = load i64, ptr %init_cnt_68, align 8, !tbaa !1
  %init_cnt_70 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %init_x0_71 = load float, ptr %init_cnt_70, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_72 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
  %init_x1_73 = load float, ptr %init_cnt_72, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_74 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %init_x2_75 = load float, ptr %init_cnt_74, align 4, !tbaa !3
  %init_cnt_76 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
  %init_count_77 = load i64, ptr %init_cnt_76, align 8
  br label %loop_hdr
loop_hdr:
  %pi_cnt_78 = phi i64 [ %init_count_77, %pre_phi ], [ %pn_cnt_78, %latch ]
  %phi_x2 = phi float [ %init_x2_75, %pre_phi ], [ %be_x2, %latch ]
  %phi_p21 = phi float [ %init_p21_65, %pre_phi ], [ %be_p21, %latch ]
  %phi_p01 = phi float [ %init_p01_53, %pre_phi ], [ %be_p01, %latch ]
  %phi_total = phi i64 [ %init_total_69, %pre_phi ], [ %be_total, %latch ]
  %phi_x1 = phi float [ %init_x1_73, %pre_phi ], [ %be_x1, %latch ]
  %phi_p10 = phi float [ %init_p10_57, %pre_phi ], [ %be_p10, %latch ]
  %phi_p12 = phi float [ %init_p12_61, %pre_phi ], [ %be_p12, %latch ]
  %phi_cycle_count = phi i64 [ %init_cycle_count_49, %pre_phi ], [ %be_cycle_count, %latch ]
  %phi_p20 = phi float [ %init_p20_63, %pre_phi ], [ %be_p20, %latch ]
  %phi_p11 = phi float [ %init_p11_59, %pre_phi ], [ %be_p11, %latch ]
  %phi_p22 = phi float [ %init_p22_67, %pre_phi ], [ %be_p22, %latch ]
  %phi_p02 = phi float [ %init_p02_55, %pre_phi ], [ %be_p02, %latch ]
  %phi_x0 = phi float [ %init_x0_71, %pre_phi ], [ %be_x0, %latch ]
  %phi_p00 = phi float [ %init_p00_51, %pre_phi ], [ %be_p00, %latch ]
  %phi_count = phi i64 [ %init_count_77, %pre_phi ], [ %be_count, %latch ]
  %cmp_hdr_79 = icmp slt i64 %pi_cnt_78, %cnt_bound_16
  br i1 %cmp_hdr_79, label %body, label %done
body:
  %il84 = load float, float* @A00, align 4
%t86 = fmul fast float %il84, %phi_x0
  %il89 = load float, float* @A01, align 4
%t91 = fmul fast float %il89, %phi_x1
%t92 = fadd fast float %t86, %t91
  %il95 = load float, float* @A02, align 4
%t97 = fmul fast float %il95, %phi_x2
%t98 = fadd fast float %t92, %t97
  %ap_99 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %il104 = load float, float* @A10, align 4
%t106 = fmul fast float %il104, %phi_x0
  %il109 = load float, float* @A11, align 4
%t111 = fmul fast float %il109, %phi_x1
%t112 = fadd fast float %t106, %t111
  %il115 = load float, float* @A12, align 4
%t117 = fmul fast float %il115, %phi_x2
%t118 = fadd fast float %t112, %t117
  %ap_119 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
  %il124 = load float, float* @A20, align 4
%t126 = fmul fast float %il124, %phi_x0
  %il129 = load float, float* @A21, align 4
%t131 = fmul fast float %il129, %phi_x1
%t132 = fadd fast float %t126, %t131
  %il135 = load float, float* @A22, align 4
%t137 = fmul fast float %il135, %phi_x2
%t138 = fadd fast float %t132, %t137
  %ap_139 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %il143 = load float, float* @Q00, align 4
%t144 = fadd fast float %phi_p00, %il143
  %ap_145 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %il149 = load float, float* @Q01, align 4
%t150 = fadd fast float %phi_p01, %il149
  %ap_151 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %il155 = load float, float* @Q02, align 4
%t156 = fadd fast float %phi_p02, %il155
  %ap_157 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %il161 = load float, float* @Q10, align 4
%t162 = fadd fast float %phi_p10, %il161
  %ap_163 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %il167 = load float, float* @Q11, align 4
%t168 = fadd fast float %phi_p11, %il167
  %ap_169 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
  %il173 = load float, float* @Q12, align 4
%t174 = fadd fast float %phi_p12, %il173
  %ap_175 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
  %il179 = load float, float* @Q20, align 4
%t180 = fadd fast float %phi_p20, %il179
  %ap_181 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
  %il185 = load float, float* @Q21, align 4
%t186 = fadd fast float %phi_p21, %il185
  %ap_187 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
  %il191 = load float, float* @Q22, align 4
%t192 = fadd fast float %phi_p22, %il191
  %ap_193 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
  %t195 = add i64 0, %phi_count
%t197 = add i64 0, 1
%t198 = add nsw i64 %t195, %t197
  %ap_199 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
  store i64 %t198, ptr %ap_199, align 8, !tbaa !1
  %t202 = add i64 0, %t198
%t204 = add i64 0, 5000000
%t205 = srem i64 %t202, %t204
%t207 = add i64 0, 0
%c208 = icmp eq i64 %t205, %t207
  br i1 %c208, label %g209_t, label %g209_e
  g209_t:
%t220 = fadd fast float %t144, %t150
%t222 = fadd fast float %t220, %t156
%t224 = fadd fast float %t222, %t162
%t226 = fadd fast float %t224, %t168
%t228 = fadd fast float %t226, %t174
%t230 = fadd fast float %t228, %t180
%t232 = fadd fast float %t230, %t186
%t234 = fadd fast float %t232, %t192
    ; let trace = %t234
%t241 = fadd fast float %phi_x0, %phi_x1
%t243 = fadd fast float %t241, %phi_x2
%t245 = fadd fast float %t243, %t234
    %pfd246 = fpext float %t245 to double
    %pso247 = load volatile ptr, ptr @stdout
    %pff248 = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0
    %ppf249 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso247, ptr %pff248, double %pfd246)
    %t235 = zext i32 %ppf249 to i64
    br label %g209_tx
  g209_tx:
    br label %g209_e
  g209_e:
  br label %latch
latch:
  %pn_cnt_78 = add i64 %pi_cnt_78, 1
  %be_cycle_count = add i64 0, %phi_cycle_count
  %be_p00 = fadd float %t144, 0.0
  %be_p01 = fadd float %t150, 0.0
  %be_p02 = fadd float %t156, 0.0
  %be_p10 = fadd float %t162, 0.0
  %be_p11 = fadd float %t168, 0.0
  %be_p12 = fadd float %t174, 0.0
  %be_p20 = fadd float %t180, 0.0
  %be_p21 = fadd float %t186, 0.0
  %be_p22 = fadd float %t192, 0.0
  %be_total = add i64 0, %phi_total
  %be_x0 = fadd float %t98, 0.0
  %be_x1 = fadd float %t118, 0.0
  %be_x2 = fadd float %t138, 0.0
  %be_count = add i64 0, %pn_cnt_78
  br label %loop_hdr, !llvm.loop !100
done:
  %arr4 = load ptr, ptr %arbase3, align 8
  store i8* %arr4, ptr %arptr3, align 8
  ret i32 0
}

; Loop metadata
!101 = !{!"llvm.loop.vectorize.enable", i1 true}
!100 = !{!100, !101}

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

!0 = !{!"Briev"}
!1 = !{!"Int", !0}
!2 = !{!"Char", !0}
!3 = !{!"Float", !0}
!4 = !{!"Bool", !0}
!5 = !{!"String", !0}
!99 = distinct !{} ; StateAliasScope
