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
@m0 = constant float bitcast (i32 1109256678 to float)
@m4 = constant float bitcast (i32 990201755 to float)
@solar_mass = alias float, float* @m0
@m2 = constant float bitcast (i32 1010362952 to float)
@pi = constant float bitcast (i32 1078530011 to float)
@dpy = constant float bitcast (i32 1136041656 to float)
@m3 = constant float bitcast (i32 987885205 to float)
@m1 = constant float bitcast (i32 1025139887 to float)
@dt = constant float bitcast (i32 1008981770 to float)

%StateChunk0 = type { i64, i64, float, float, float, float, float, float, float, float, float, float, float, float, float }
%StateChunk1 = type { float, float, float, float, float, float, float, float, float, float, float, float, float, float, float }
%StateChunk2 = type { float, float, float, i64 }
%State = type { i64, i64, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, i64 }
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

define void @simulate(ptr noalias nocapture align 8 %state) local_unnamed_addr #4 alwaysinline {
  entry:
  %fdp2 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t1 = load i64, i64* %fdp2, align 8
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 8
%c5 = icmp slt i64 %t1, %t3
  %pi6 = and i1 %c5, true
  br i1 %pi6, label %ps8, label %pp7
  pp7:
    unreachable
  ps8:
  call void @llvm.assume(i1 %pi6)
  %fdp11 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t10 = load float, ptr %fdp11, align 4
  %fdp13 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t12 = load float, ptr %fdp13, align 4
%t14 = fsub fast float %t10, %t12
  ; let dx01 = %t14
  %fdp17 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t16 = load float, ptr %fdp17, align 4
  %fdp19 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t18 = load float, ptr %fdp19, align 4
%t20 = fsub fast float %t16, %t18
  ; let dy01 = %t20
  %fdp23 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t22 = load float, ptr %fdp23, align 4
  %fdp25 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t24 = load float, ptr %fdp25, align 4
%t26 = fsub fast float %t22, %t24
  ; let dz01 = %t26
  %fdp29 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t28 = load float, ptr %fdp29, align 4
  %fdp31 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t30 = load float, ptr %fdp31, align 4
%t32 = fsub fast float %t28, %t30
  ; let dx02 = %t32
  %fdp35 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t34 = load float, ptr %fdp35, align 4
  %fdp37 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t36 = load float, ptr %fdp37, align 4
%t38 = fsub fast float %t34, %t36
  ; let dy02 = %t38
  %fdp41 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t40 = load float, ptr %fdp41, align 4
  %fdp43 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t42 = load float, ptr %fdp43, align 4
%t44 = fsub fast float %t40, %t42
  ; let dz02 = %t44
  %fdp47 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t46 = load float, ptr %fdp47, align 4
  %fdp49 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t48 = load float, ptr %fdp49, align 4
%t50 = fsub fast float %t46, %t48
  ; let dx03 = %t50
  %fdp53 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t52 = load float, ptr %fdp53, align 4
  %fdp55 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t54 = load float, ptr %fdp55, align 4
%t56 = fsub fast float %t52, %t54
  ; let dy03 = %t56
  %fdp59 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t58 = load float, ptr %fdp59, align 4
  %fdp61 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t60 = load float, ptr %fdp61, align 4
%t62 = fsub fast float %t58, %t60
  ; let dz03 = %t62
  %fdp65 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t64 = load float, ptr %fdp65, align 4
  %fdp67 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t66 = load float, ptr %fdp67, align 4
%t68 = fsub fast float %t64, %t66
  ; let dx04 = %t68
  %fdp71 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t70 = load float, ptr %fdp71, align 4
  %fdp73 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t72 = load float, ptr %fdp73, align 4
%t74 = fsub fast float %t70, %t72
  ; let dy04 = %t74
  %fdp77 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t76 = load float, ptr %fdp77, align 4
  %fdp79 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t78 = load float, ptr %fdp79, align 4
%t80 = fsub fast float %t76, %t78
  ; let dz04 = %t80
  %fdp83 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t82 = load float, ptr %fdp83, align 4
  %fdp85 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t84 = load float, ptr %fdp85, align 4
%t86 = fsub fast float %t82, %t84
  ; let dx12 = %t86
  %fdp89 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t88 = load float, ptr %fdp89, align 4
  %fdp91 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t90 = load float, ptr %fdp91, align 4
%t92 = fsub fast float %t88, %t90
  ; let dy12 = %t92
  %fdp95 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t94 = load float, ptr %fdp95, align 4
  %fdp97 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t96 = load float, ptr %fdp97, align 4
%t98 = fsub fast float %t94, %t96
  ; let dz12 = %t98
  %fdp101 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t100 = load float, ptr %fdp101, align 4
  %fdp103 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t102 = load float, ptr %fdp103, align 4
%t104 = fsub fast float %t100, %t102
  ; let dx13 = %t104
  %fdp107 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t106 = load float, ptr %fdp107, align 4
  %fdp109 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t108 = load float, ptr %fdp109, align 4
%t110 = fsub fast float %t106, %t108
  ; let dy13 = %t110
  %fdp113 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t112 = load float, ptr %fdp113, align 4
  %fdp115 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t114 = load float, ptr %fdp115, align 4
%t116 = fsub fast float %t112, %t114
  ; let dz13 = %t116
  %fdp119 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t118 = load float, ptr %fdp119, align 4
  %fdp121 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t120 = load float, ptr %fdp121, align 4
%t122 = fsub fast float %t118, %t120
  ; let dx14 = %t122
  %fdp125 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t124 = load float, ptr %fdp125, align 4
  %fdp127 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t126 = load float, ptr %fdp127, align 4
%t128 = fsub fast float %t124, %t126
  ; let dy14 = %t128
  %fdp131 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t130 = load float, ptr %fdp131, align 4
  %fdp133 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t132 = load float, ptr %fdp133, align 4
%t134 = fsub fast float %t130, %t132
  ; let dz14 = %t134
  %fdp137 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t136 = load float, ptr %fdp137, align 4
  %fdp139 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t138 = load float, ptr %fdp139, align 4
%t140 = fsub fast float %t136, %t138
  ; let dx23 = %t140
  %fdp143 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t142 = load float, ptr %fdp143, align 4
  %fdp145 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t144 = load float, ptr %fdp145, align 4
%t146 = fsub fast float %t142, %t144
  ; let dy23 = %t146
  %fdp149 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t148 = load float, ptr %fdp149, align 4
  %fdp151 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t150 = load float, ptr %fdp151, align 4
%t152 = fsub fast float %t148, %t150
  ; let dz23 = %t152
  %fdp155 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t154 = load float, ptr %fdp155, align 4
  %fdp157 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t156 = load float, ptr %fdp157, align 4
%t158 = fsub fast float %t154, %t156
  ; let dx24 = %t158
  %fdp161 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t160 = load float, ptr %fdp161, align 4
  %fdp163 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t162 = load float, ptr %fdp163, align 4
%t164 = fsub fast float %t160, %t162
  ; let dy24 = %t164
  %fdp167 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t166 = load float, ptr %fdp167, align 4
  %fdp169 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t168 = load float, ptr %fdp169, align 4
%t170 = fsub fast float %t166, %t168
  ; let dz24 = %t170
  %fdp173 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t172 = load float, ptr %fdp173, align 4
  %fdp175 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t174 = load float, ptr %fdp175, align 4
%t176 = fsub fast float %t172, %t174
  ; let dx34 = %t176
  %fdp179 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t178 = load float, ptr %fdp179, align 4
  %fdp181 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t180 = load float, ptr %fdp181, align 4
%t182 = fsub fast float %t178, %t180
  ; let dy34 = %t182
  %fdp185 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t184 = load float, ptr %fdp185, align 4
  %fdp187 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t186 = load float, ptr %fdp187, align 4
%t188 = fsub fast float %t184, %t186
  ; let dz34 = %t188
  %fdp191 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t190 = load i64, i64* %fdp191, align 8
%t193 = add i64 0, 1
%t194 = add nsw i64 %t190, %t193
  %ap_195 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 %t194, ptr %ap_195, align 8, !tbaa !1
%t201 = fmul fast float %t14, %t14
%t205 = fmul fast float %t20, %t20
%t206 = fadd fast float %t201, %t205
%t210 = fmul fast float %t26, %t26
%t211 = fadd fast float %t206, %t210
  ; let dsq01 = %t211
%t217 = fmul fast float %t32, %t32
%t221 = fmul fast float %t38, %t38
%t222 = fadd fast float %t217, %t221
%t226 = fmul fast float %t44, %t44
%t227 = fadd fast float %t222, %t226
  ; let dsq02 = %t227
%t233 = fmul fast float %t50, %t50
%t237 = fmul fast float %t56, %t56
%t238 = fadd fast float %t233, %t237
%t242 = fmul fast float %t62, %t62
%t243 = fadd fast float %t238, %t242
  ; let dsq03 = %t243
%t249 = fmul fast float %t68, %t68
%t253 = fmul fast float %t74, %t74
%t254 = fadd fast float %t249, %t253
%t258 = fmul fast float %t80, %t80
%t259 = fadd fast float %t254, %t258
  ; let dsq04 = %t259
%t265 = fmul fast float %t86, %t86
%t269 = fmul fast float %t92, %t92
%t270 = fadd fast float %t265, %t269
%t274 = fmul fast float %t98, %t98
%t275 = fadd fast float %t270, %t274
  ; let dsq12 = %t275
%t281 = fmul fast float %t104, %t104
%t285 = fmul fast float %t110, %t110
%t286 = fadd fast float %t281, %t285
%t290 = fmul fast float %t116, %t116
%t291 = fadd fast float %t286, %t290
  ; let dsq13 = %t291
%t297 = fmul fast float %t122, %t122
%t301 = fmul fast float %t128, %t128
%t302 = fadd fast float %t297, %t301
%t306 = fmul fast float %t134, %t134
%t307 = fadd fast float %t302, %t306
  ; let dsq14 = %t307
%t313 = fmul fast float %t140, %t140
%t317 = fmul fast float %t146, %t146
%t318 = fadd fast float %t313, %t317
%t322 = fmul fast float %t152, %t152
%t323 = fadd fast float %t318, %t322
  ; let dsq23 = %t323
%t329 = fmul fast float %t158, %t158
%t333 = fmul fast float %t164, %t164
%t334 = fadd fast float %t329, %t333
%t338 = fmul fast float %t170, %t170
%t339 = fadd fast float %t334, %t338
  ; let dsq24 = %t339
%t345 = fmul fast float %t176, %t176
%t349 = fmul fast float %t182, %t182
%t350 = fadd fast float %t345, %t349
%t354 = fmul fast float %t188, %t188
%t355 = fadd fast float %t350, %t354
  ; let dsq34 = %t355
%ff360 = bitcast i32 1056964608 to float
%t361 = fmul fast float %t211, %ff360
  ; let dist01a = %t361
%ff366 = bitcast i32 1056964608 to float
%t367 = fmul fast float %t227, %ff366
  ; let dist02a = %t367
%ff372 = bitcast i32 1056964608 to float
%t373 = fmul fast float %t243, %ff372
  ; let dist03a = %t373
%ff378 = bitcast i32 1056964608 to float
%t379 = fmul fast float %t259, %ff378
  ; let dist04a = %t379
%ff384 = bitcast i32 1056964608 to float
%t385 = fmul fast float %t275, %ff384
  ; let dist12a = %t385
%ff390 = bitcast i32 1056964608 to float
%t391 = fmul fast float %t291, %ff390
  ; let dist13a = %t391
%ff396 = bitcast i32 1056964608 to float
%t397 = fmul fast float %t307, %ff396
  ; let dist14a = %t397
%ff402 = bitcast i32 1056964608 to float
%t403 = fmul fast float %t323, %ff402
  ; let dist23a = %t403
%ff408 = bitcast i32 1056964608 to float
%t409 = fmul fast float %t339, %ff408
  ; let dist24a = %t409
%ff414 = bitcast i32 1056964608 to float
%t415 = fmul fast float %t355, %ff414
  ; let dist34a = %t415
%ff419 = bitcast i32 1056964608 to float
%t425 = fdiv fast float %t211, %t361
%t426 = fadd fast float %t361, %t425
%t427 = fmul fast float %ff419, %t426
  ; let dist01b = %t427
%ff431 = bitcast i32 1056964608 to float
%t437 = fdiv fast float %t227, %t367
%t438 = fadd fast float %t367, %t437
%t439 = fmul fast float %ff431, %t438
  ; let dist02b = %t439
%ff443 = bitcast i32 1056964608 to float
%t449 = fdiv fast float %t243, %t373
%t450 = fadd fast float %t373, %t449
%t451 = fmul fast float %ff443, %t450
  ; let dist03b = %t451
%ff455 = bitcast i32 1056964608 to float
%t461 = fdiv fast float %t259, %t379
%t462 = fadd fast float %t379, %t461
%t463 = fmul fast float %ff455, %t462
  ; let dist04b = %t463
%ff467 = bitcast i32 1056964608 to float
%t473 = fdiv fast float %t275, %t385
%t474 = fadd fast float %t385, %t473
%t475 = fmul fast float %ff467, %t474
  ; let dist12b = %t475
%ff479 = bitcast i32 1056964608 to float
%t485 = fdiv fast float %t291, %t391
%t486 = fadd fast float %t391, %t485
%t487 = fmul fast float %ff479, %t486
  ; let dist13b = %t487
%ff491 = bitcast i32 1056964608 to float
%t497 = fdiv fast float %t307, %t397
%t498 = fadd fast float %t397, %t497
%t499 = fmul fast float %ff491, %t498
  ; let dist14b = %t499
%ff503 = bitcast i32 1056964608 to float
%t509 = fdiv fast float %t323, %t403
%t510 = fadd fast float %t403, %t509
%t511 = fmul fast float %ff503, %t510
  ; let dist23b = %t511
%ff515 = bitcast i32 1056964608 to float
%t521 = fdiv fast float %t339, %t409
%t522 = fadd fast float %t409, %t521
%t523 = fmul fast float %ff515, %t522
  ; let dist24b = %t523
%ff527 = bitcast i32 1056964608 to float
%t533 = fdiv fast float %t355, %t415
%t534 = fadd fast float %t415, %t533
%t535 = fmul fast float %ff527, %t534
  ; let dist34b = %t535
%ff539 = bitcast i32 1056964608 to float
%t545 = fdiv fast float %t211, %t427
%t546 = fadd fast float %t427, %t545
%t547 = fmul fast float %ff539, %t546
  ; let dist01c = %t547
%ff551 = bitcast i32 1056964608 to float
%t557 = fdiv fast float %t227, %t439
%t558 = fadd fast float %t439, %t557
%t559 = fmul fast float %ff551, %t558
  ; let dist02c = %t559
%ff563 = bitcast i32 1056964608 to float
%t569 = fdiv fast float %t243, %t451
%t570 = fadd fast float %t451, %t569
%t571 = fmul fast float %ff563, %t570
  ; let dist03c = %t571
%ff575 = bitcast i32 1056964608 to float
%t581 = fdiv fast float %t259, %t463
%t582 = fadd fast float %t463, %t581
%t583 = fmul fast float %ff575, %t582
  ; let dist04c = %t583
%ff587 = bitcast i32 1056964608 to float
%t593 = fdiv fast float %t275, %t475
%t594 = fadd fast float %t475, %t593
%t595 = fmul fast float %ff587, %t594
  ; let dist12c = %t595
%ff599 = bitcast i32 1056964608 to float
%t605 = fdiv fast float %t291, %t487
%t606 = fadd fast float %t487, %t605
%t607 = fmul fast float %ff599, %t606
  ; let dist13c = %t607
%ff611 = bitcast i32 1056964608 to float
%t617 = fdiv fast float %t307, %t499
%t618 = fadd fast float %t499, %t617
%t619 = fmul fast float %ff611, %t618
  ; let dist14c = %t619
%ff623 = bitcast i32 1056964608 to float
%t629 = fdiv fast float %t323, %t511
%t630 = fadd fast float %t511, %t629
%t631 = fmul fast float %ff623, %t630
  ; let dist23c = %t631
%ff635 = bitcast i32 1056964608 to float
%t641 = fdiv fast float %t339, %t523
%t642 = fadd fast float %t523, %t641
%t643 = fmul fast float %ff635, %t642
  ; let dist24c = %t643
%ff647 = bitcast i32 1056964608 to float
%t653 = fdiv fast float %t355, %t535
%t654 = fadd fast float %t535, %t653
%t655 = fmul fast float %ff647, %t654
  ; let dist34c = %t655
%ff659 = bitcast i32 1056964608 to float
%t665 = fdiv fast float %t211, %t547
%t666 = fadd fast float %t547, %t665
%t667 = fmul fast float %ff659, %t666
  ; let dist01d = %t667
%ff671 = bitcast i32 1056964608 to float
%t677 = fdiv fast float %t227, %t559
%t678 = fadd fast float %t559, %t677
%t679 = fmul fast float %ff671, %t678
  ; let dist02d = %t679
%ff683 = bitcast i32 1056964608 to float
%t689 = fdiv fast float %t243, %t571
%t690 = fadd fast float %t571, %t689
%t691 = fmul fast float %ff683, %t690
  ; let dist03d = %t691
%ff695 = bitcast i32 1056964608 to float
%t701 = fdiv fast float %t259, %t583
%t702 = fadd fast float %t583, %t701
%t703 = fmul fast float %ff695, %t702
  ; let dist04d = %t703
%ff707 = bitcast i32 1056964608 to float
%t713 = fdiv fast float %t275, %t595
%t714 = fadd fast float %t595, %t713
%t715 = fmul fast float %ff707, %t714
  ; let dist12d = %t715
%ff719 = bitcast i32 1056964608 to float
%t725 = fdiv fast float %t291, %t607
%t726 = fadd fast float %t607, %t725
%t727 = fmul fast float %ff719, %t726
  ; let dist13d = %t727
%ff731 = bitcast i32 1056964608 to float
%t737 = fdiv fast float %t307, %t619
%t738 = fadd fast float %t619, %t737
%t739 = fmul fast float %ff731, %t738
  ; let dist14d = %t739
%ff743 = bitcast i32 1056964608 to float
%t749 = fdiv fast float %t323, %t631
%t750 = fadd fast float %t631, %t749
%t751 = fmul fast float %ff743, %t750
  ; let dist23d = %t751
%ff755 = bitcast i32 1056964608 to float
%t761 = fdiv fast float %t339, %t643
%t762 = fadd fast float %t643, %t761
%t763 = fmul fast float %ff755, %t762
  ; let dist24d = %t763
%ff767 = bitcast i32 1056964608 to float
%t773 = fdiv fast float %t355, %t655
%t774 = fadd fast float %t655, %t773
%t775 = fmul fast float %ff767, %t774
  ; let dist34d = %t775
%ff779 = bitcast i32 1056964608 to float
%t785 = fdiv fast float %t211, %t667
%t786 = fadd fast float %t667, %t785
%t787 = fmul fast float %ff779, %t786
  ; let dist01e = %t787
%ff791 = bitcast i32 1056964608 to float
%t797 = fdiv fast float %t227, %t679
%t798 = fadd fast float %t679, %t797
%t799 = fmul fast float %ff791, %t798
  ; let dist02e = %t799
%ff803 = bitcast i32 1056964608 to float
%t809 = fdiv fast float %t243, %t691
%t810 = fadd fast float %t691, %t809
%t811 = fmul fast float %ff803, %t810
  ; let dist03e = %t811
%ff815 = bitcast i32 1056964608 to float
%t821 = fdiv fast float %t259, %t703
%t822 = fadd fast float %t703, %t821
%t823 = fmul fast float %ff815, %t822
  ; let dist04e = %t823
%ff827 = bitcast i32 1056964608 to float
%t833 = fdiv fast float %t275, %t715
%t834 = fadd fast float %t715, %t833
%t835 = fmul fast float %ff827, %t834
  ; let dist12e = %t835
%ff839 = bitcast i32 1056964608 to float
%t845 = fdiv fast float %t291, %t727
%t846 = fadd fast float %t727, %t845
%t847 = fmul fast float %ff839, %t846
  ; let dist13e = %t847
%ff851 = bitcast i32 1056964608 to float
%t857 = fdiv fast float %t307, %t739
%t858 = fadd fast float %t739, %t857
%t859 = fmul fast float %ff851, %t858
  ; let dist14e = %t859
%ff863 = bitcast i32 1056964608 to float
%t869 = fdiv fast float %t323, %t751
%t870 = fadd fast float %t751, %t869
%t871 = fmul fast float %ff863, %t870
  ; let dist23e = %t871
%ff875 = bitcast i32 1056964608 to float
%t881 = fdiv fast float %t339, %t763
%t882 = fadd fast float %t763, %t881
%t883 = fmul fast float %ff875, %t882
  ; let dist24e = %t883
%ff887 = bitcast i32 1056964608 to float
%t893 = fdiv fast float %t355, %t775
%t894 = fadd fast float %t775, %t893
%t895 = fmul fast float %ff887, %t894
  ; let dist34e = %t895
%ff899 = bitcast i32 1056964608 to float
%t905 = fdiv fast float %t211, %t787
%t906 = fadd fast float %t787, %t905
%t907 = fmul fast float %ff899, %t906
  ; let dist01 = %t907
%ff911 = bitcast i32 1056964608 to float
%t917 = fdiv fast float %t227, %t799
%t918 = fadd fast float %t799, %t917
%t919 = fmul fast float %ff911, %t918
  ; let dist02 = %t919
%ff923 = bitcast i32 1056964608 to float
%t929 = fdiv fast float %t243, %t811
%t930 = fadd fast float %t811, %t929
%t931 = fmul fast float %ff923, %t930
  ; let dist03 = %t931
%ff935 = bitcast i32 1056964608 to float
%t941 = fdiv fast float %t259, %t823
%t942 = fadd fast float %t823, %t941
%t943 = fmul fast float %ff935, %t942
  ; let dist04 = %t943
%ff947 = bitcast i32 1056964608 to float
%t953 = fdiv fast float %t275, %t835
%t954 = fadd fast float %t835, %t953
%t955 = fmul fast float %ff947, %t954
  ; let dist12 = %t955
%ff959 = bitcast i32 1056964608 to float
%t965 = fdiv fast float %t291, %t847
%t966 = fadd fast float %t847, %t965
%t967 = fmul fast float %ff959, %t966
  ; let dist13 = %t967
%ff971 = bitcast i32 1056964608 to float
%t977 = fdiv fast float %t307, %t859
%t978 = fadd fast float %t859, %t977
%t979 = fmul fast float %ff971, %t978
  ; let dist14 = %t979
%ff983 = bitcast i32 1056964608 to float
%t989 = fdiv fast float %t323, %t871
%t990 = fadd fast float %t871, %t989
%t991 = fmul fast float %ff983, %t990
  ; let dist23 = %t991
%ff995 = bitcast i32 1056964608 to float
%t1001 = fdiv fast float %t339, %t883
%t1002 = fadd fast float %t883, %t1001
%t1003 = fmul fast float %ff995, %t1002
  ; let dist24 = %t1003
%ff1007 = bitcast i32 1056964608 to float
%t1013 = fdiv fast float %t355, %t895
%t1014 = fadd fast float %t895, %t1013
%t1015 = fmul fast float %ff1007, %t1014
  ; let dist34 = %t1015
  %il1018 = load float, float* @dt, align 4
%t1022 = fmul fast float %t211, %t907
%t1023 = fdiv fast float %il1018, %t1022
  ; let mag01 = %t1023
  %il1026 = load float, float* @dt, align 4
%t1030 = fmul fast float %t227, %t919
%t1031 = fdiv fast float %il1026, %t1030
  ; let mag02 = %t1031
  %il1034 = load float, float* @dt, align 4
%t1038 = fmul fast float %t243, %t931
%t1039 = fdiv fast float %il1034, %t1038
  ; let mag03 = %t1039
  %il1042 = load float, float* @dt, align 4
%t1046 = fmul fast float %t259, %t943
%t1047 = fdiv fast float %il1042, %t1046
  ; let mag04 = %t1047
  %il1050 = load float, float* @dt, align 4
%t1054 = fmul fast float %t275, %t955
%t1055 = fdiv fast float %il1050, %t1054
  ; let mag12 = %t1055
  %il1058 = load float, float* @dt, align 4
%t1062 = fmul fast float %t291, %t967
%t1063 = fdiv fast float %il1058, %t1062
  ; let mag13 = %t1063
  %il1066 = load float, float* @dt, align 4
%t1070 = fmul fast float %t307, %t979
%t1071 = fdiv fast float %il1066, %t1070
  ; let mag14 = %t1071
  %il1074 = load float, float* @dt, align 4
%t1078 = fmul fast float %t323, %t991
%t1079 = fdiv fast float %il1074, %t1078
  ; let mag23 = %t1079
  %il1082 = load float, float* @dt, align 4
%t1086 = fmul fast float %t339, %t1003
%t1087 = fdiv fast float %il1082, %t1086
  ; let mag24 = %t1087
  %il1090 = load float, float* @dt, align 4
%t1094 = fmul fast float %t355, %t1015
%t1095 = fdiv fast float %il1090, %t1094
  ; let mag34 = %t1095
  %fdp1101 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t1100 = load float, ptr %fdp1101, align 4
  %il1106 = load float, float* @m1, align 4
%t1107 = fmul fast float %t14, %il1106
%t1109 = fmul fast float %t1107, %t1023
%t1110 = fsub fast float %t1100, %t1109
  %il1115 = load float, float* @m2, align 4
%t1116 = fmul fast float %t32, %il1115
%t1118 = fmul fast float %t1116, %t1031
%t1119 = fsub fast float %t1110, %t1118
  %il1124 = load float, float* @m3, align 4
%t1125 = fmul fast float %t50, %il1124
%t1127 = fmul fast float %t1125, %t1039
%t1128 = fsub fast float %t1119, %t1127
  %il1133 = load float, float* @m4, align 4
%t1134 = fmul fast float %t68, %il1133
%t1136 = fmul fast float %t1134, %t1047
%t1137 = fsub fast float %t1128, %t1136
  ; let nvx0 = %t1137
  %fdp1143 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t1142 = load float, ptr %fdp1143, align 4
  %il1148 = load float, float* @m1, align 4
%t1149 = fmul fast float %t26, %il1148
%t1151 = fmul fast float %t1149, %t1023
%t1152 = fsub fast float %t1142, %t1151
  %il1157 = load float, float* @m2, align 4
%t1158 = fmul fast float %t44, %il1157
%t1160 = fmul fast float %t1158, %t1031
%t1161 = fsub fast float %t1152, %t1160
  %il1166 = load float, float* @m3, align 4
%t1167 = fmul fast float %t62, %il1166
%t1169 = fmul fast float %t1167, %t1039
%t1170 = fsub fast float %t1161, %t1169
  %il1175 = load float, float* @m4, align 4
%t1176 = fmul fast float %t80, %il1175
%t1178 = fmul fast float %t1176, %t1047
%t1179 = fsub fast float %t1170, %t1178
  ; let nvz0 = %t1179
  %fdp1185 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t1184 = load float, ptr %fdp1185, align 4
  %il1190 = load float, float* @m1, align 4
%t1191 = fmul fast float %t20, %il1190
%t1193 = fmul fast float %t1191, %t1023
%t1194 = fsub fast float %t1184, %t1193
  %il1199 = load float, float* @m2, align 4
%t1200 = fmul fast float %t38, %il1199
%t1202 = fmul fast float %t1200, %t1031
%t1203 = fsub fast float %t1194, %t1202
  %il1208 = load float, float* @m3, align 4
%t1209 = fmul fast float %t56, %il1208
%t1211 = fmul fast float %t1209, %t1039
%t1212 = fsub fast float %t1203, %t1211
  %il1217 = load float, float* @m4, align 4
%t1218 = fmul fast float %t74, %il1217
%t1220 = fmul fast float %t1218, %t1047
%t1221 = fsub fast float %t1212, %t1220
  ; let nvy0 = %t1221
  %fdp1227 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t1226 = load float, ptr %fdp1227, align 4
  %il1232 = load float, float* @m0, align 4
%t1233 = fmul fast float %t20, %il1232
%t1235 = fmul fast float %t1233, %t1023
%t1236 = fadd fast float %t1226, %t1235
  %il1241 = load float, float* @m2, align 4
%t1242 = fmul fast float %t92, %il1241
%t1244 = fmul fast float %t1242, %t1055
%t1245 = fsub fast float %t1236, %t1244
  %il1250 = load float, float* @m3, align 4
%t1251 = fmul fast float %t110, %il1250
%t1253 = fmul fast float %t1251, %t1063
%t1254 = fsub fast float %t1245, %t1253
  %il1259 = load float, float* @m4, align 4
%t1260 = fmul fast float %t128, %il1259
%t1262 = fmul fast float %t1260, %t1071
%t1263 = fsub fast float %t1254, %t1262
  ; let nvy1 = %t1263
  %fdp1269 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t1268 = load float, ptr %fdp1269, align 4
  %il1274 = load float, float* @m0, align 4
%t1275 = fmul fast float %t14, %il1274
%t1277 = fmul fast float %t1275, %t1023
%t1278 = fadd fast float %t1268, %t1277
  %il1283 = load float, float* @m2, align 4
%t1284 = fmul fast float %t86, %il1283
%t1286 = fmul fast float %t1284, %t1055
%t1287 = fsub fast float %t1278, %t1286
  %il1292 = load float, float* @m3, align 4
%t1293 = fmul fast float %t104, %il1292
%t1295 = fmul fast float %t1293, %t1063
%t1296 = fsub fast float %t1287, %t1295
  %il1301 = load float, float* @m4, align 4
%t1302 = fmul fast float %t122, %il1301
%t1304 = fmul fast float %t1302, %t1071
%t1305 = fsub fast float %t1296, %t1304
  ; let nvx1 = %t1305
  %fdp1311 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t1310 = load float, ptr %fdp1311, align 4
  %il1316 = load float, float* @m0, align 4
%t1317 = fmul fast float %t26, %il1316
%t1319 = fmul fast float %t1317, %t1023
%t1320 = fadd fast float %t1310, %t1319
  %il1325 = load float, float* @m2, align 4
%t1326 = fmul fast float %t98, %il1325
%t1328 = fmul fast float %t1326, %t1055
%t1329 = fsub fast float %t1320, %t1328
  %il1334 = load float, float* @m3, align 4
%t1335 = fmul fast float %t116, %il1334
%t1337 = fmul fast float %t1335, %t1063
%t1338 = fsub fast float %t1329, %t1337
  %il1343 = load float, float* @m4, align 4
%t1344 = fmul fast float %t134, %il1343
%t1346 = fmul fast float %t1344, %t1071
%t1347 = fsub fast float %t1338, %t1346
  ; let nvz1 = %t1347
  %fdp1353 = getelementptr inbounds %State, ptr %state, i32 0, i32 18
  %t1352 = load float, ptr %fdp1353, align 4
  %il1358 = load float, float* @m0, align 4
%t1359 = fmul fast float %t38, %il1358
%t1361 = fmul fast float %t1359, %t1031
%t1362 = fadd fast float %t1352, %t1361
  %il1367 = load float, float* @m1, align 4
%t1368 = fmul fast float %t92, %il1367
%t1370 = fmul fast float %t1368, %t1055
%t1371 = fadd fast float %t1362, %t1370
  %il1376 = load float, float* @m3, align 4
%t1377 = fmul fast float %t146, %il1376
%t1379 = fmul fast float %t1377, %t1079
%t1380 = fsub fast float %t1371, %t1379
  %il1385 = load float, float* @m4, align 4
%t1386 = fmul fast float %t164, %il1385
%t1388 = fmul fast float %t1386, %t1087
%t1389 = fsub fast float %t1380, %t1388
  ; let nvy2 = %t1389
  %fdp1395 = getelementptr inbounds %State, ptr %state, i32 0, i32 19
  %t1394 = load float, ptr %fdp1395, align 4
  %il1400 = load float, float* @m0, align 4
%t1401 = fmul fast float %t44, %il1400
%t1403 = fmul fast float %t1401, %t1031
%t1404 = fadd fast float %t1394, %t1403
  %il1409 = load float, float* @m1, align 4
%t1410 = fmul fast float %t98, %il1409
%t1412 = fmul fast float %t1410, %t1055
%t1413 = fadd fast float %t1404, %t1412
  %il1418 = load float, float* @m3, align 4
%t1419 = fmul fast float %t152, %il1418
%t1421 = fmul fast float %t1419, %t1079
%t1422 = fsub fast float %t1413, %t1421
  %il1427 = load float, float* @m4, align 4
%t1428 = fmul fast float %t170, %il1427
%t1430 = fmul fast float %t1428, %t1087
%t1431 = fsub fast float %t1422, %t1430
  ; let nvz2 = %t1431
  %fdp1437 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
  %t1436 = load float, ptr %fdp1437, align 4
  %il1442 = load float, float* @m0, align 4
%t1443 = fmul fast float %t32, %il1442
%t1445 = fmul fast float %t1443, %t1031
%t1446 = fadd fast float %t1436, %t1445
  %il1451 = load float, float* @m1, align 4
%t1452 = fmul fast float %t86, %il1451
%t1454 = fmul fast float %t1452, %t1055
%t1455 = fadd fast float %t1446, %t1454
  %il1460 = load float, float* @m3, align 4
%t1461 = fmul fast float %t140, %il1460
%t1463 = fmul fast float %t1461, %t1079
%t1464 = fsub fast float %t1455, %t1463
  %il1469 = load float, float* @m4, align 4
%t1470 = fmul fast float %t158, %il1469
%t1472 = fmul fast float %t1470, %t1087
%t1473 = fsub fast float %t1464, %t1472
  ; let nvx2 = %t1473
  %fdp1479 = getelementptr inbounds %State, ptr %state, i32 0, i32 30
  %t1478 = load float, ptr %fdp1479, align 4
  %il1484 = load float, float* @m0, align 4
%t1485 = fmul fast float %t74, %il1484
%t1487 = fmul fast float %t1485, %t1047
%t1488 = fadd fast float %t1478, %t1487
  %il1493 = load float, float* @m1, align 4
%t1494 = fmul fast float %t128, %il1493
%t1496 = fmul fast float %t1494, %t1071
%t1497 = fadd fast float %t1488, %t1496
  %il1502 = load float, float* @m2, align 4
%t1503 = fmul fast float %t164, %il1502
%t1505 = fmul fast float %t1503, %t1087
%t1506 = fadd fast float %t1497, %t1505
  %il1511 = load float, float* @m3, align 4
%t1512 = fmul fast float %t182, %il1511
%t1514 = fmul fast float %t1512, %t1095
%t1515 = fadd fast float %t1506, %t1514
  ; let nvy4 = %t1515
  %fdp1521 = getelementptr inbounds %State, ptr %state, i32 0, i32 29
  %t1520 = load float, ptr %fdp1521, align 4
  %il1526 = load float, float* @m0, align 4
%t1527 = fmul fast float %t68, %il1526
%t1529 = fmul fast float %t1527, %t1047
%t1530 = fadd fast float %t1520, %t1529
  %il1535 = load float, float* @m1, align 4
%t1536 = fmul fast float %t122, %il1535
%t1538 = fmul fast float %t1536, %t1071
%t1539 = fadd fast float %t1530, %t1538
  %il1544 = load float, float* @m2, align 4
%t1545 = fmul fast float %t158, %il1544
%t1547 = fmul fast float %t1545, %t1087
%t1548 = fadd fast float %t1539, %t1547
  %il1553 = load float, float* @m3, align 4
%t1554 = fmul fast float %t176, %il1553
%t1556 = fmul fast float %t1554, %t1095
%t1557 = fadd fast float %t1548, %t1556
  ; let nvx4 = %t1557
  %fdp1563 = getelementptr inbounds %State, ptr %state, i32 0, i32 31
  %t1562 = load float, ptr %fdp1563, align 4
  %il1568 = load float, float* @m0, align 4
%t1569 = fmul fast float %t80, %il1568
%t1571 = fmul fast float %t1569, %t1047
%t1572 = fadd fast float %t1562, %t1571
  %il1577 = load float, float* @m1, align 4
%t1578 = fmul fast float %t134, %il1577
%t1580 = fmul fast float %t1578, %t1071
%t1581 = fadd fast float %t1572, %t1580
  %il1586 = load float, float* @m2, align 4
%t1587 = fmul fast float %t170, %il1586
%t1589 = fmul fast float %t1587, %t1087
%t1590 = fadd fast float %t1581, %t1589
  %il1595 = load float, float* @m3, align 4
%t1596 = fmul fast float %t188, %il1595
%t1598 = fmul fast float %t1596, %t1095
%t1599 = fadd fast float %t1590, %t1598
  ; let nvz4 = %t1599
  %fdp1605 = getelementptr inbounds %State, ptr %state, i32 0, i32 24
  %t1604 = load float, ptr %fdp1605, align 4
  %il1610 = load float, float* @m0, align 4
%t1611 = fmul fast float %t56, %il1610
%t1613 = fmul fast float %t1611, %t1039
%t1614 = fadd fast float %t1604, %t1613
  %il1619 = load float, float* @m1, align 4
%t1620 = fmul fast float %t110, %il1619
%t1622 = fmul fast float %t1620, %t1063
%t1623 = fadd fast float %t1614, %t1622
  %il1628 = load float, float* @m2, align 4
%t1629 = fmul fast float %t146, %il1628
%t1631 = fmul fast float %t1629, %t1079
%t1632 = fadd fast float %t1623, %t1631
  %il1637 = load float, float* @m4, align 4
%t1638 = fmul fast float %t182, %il1637
%t1640 = fmul fast float %t1638, %t1095
%t1641 = fsub fast float %t1632, %t1640
  ; let nvy3 = %t1641
  %fdp1647 = getelementptr inbounds %State, ptr %state, i32 0, i32 25
  %t1646 = load float, ptr %fdp1647, align 4
  %il1652 = load float, float* @m0, align 4
%t1653 = fmul fast float %t62, %il1652
%t1655 = fmul fast float %t1653, %t1039
%t1656 = fadd fast float %t1646, %t1655
  %il1661 = load float, float* @m1, align 4
%t1662 = fmul fast float %t116, %il1661
%t1664 = fmul fast float %t1662, %t1063
%t1665 = fadd fast float %t1656, %t1664
  %il1670 = load float, float* @m2, align 4
%t1671 = fmul fast float %t152, %il1670
%t1673 = fmul fast float %t1671, %t1079
%t1674 = fadd fast float %t1665, %t1673
  %il1679 = load float, float* @m4, align 4
%t1680 = fmul fast float %t188, %il1679
%t1682 = fmul fast float %t1680, %t1095
%t1683 = fsub fast float %t1674, %t1682
  ; let nvz3 = %t1683
  %fdp1689 = getelementptr inbounds %State, ptr %state, i32 0, i32 23
  %t1688 = load float, ptr %fdp1689, align 4
  %il1694 = load float, float* @m0, align 4
%t1695 = fmul fast float %t50, %il1694
%t1697 = fmul fast float %t1695, %t1039
%t1698 = fadd fast float %t1688, %t1697
  %il1703 = load float, float* @m1, align 4
%t1704 = fmul fast float %t104, %il1703
%t1706 = fmul fast float %t1704, %t1063
%t1707 = fadd fast float %t1698, %t1706
  %il1712 = load float, float* @m2, align 4
%t1713 = fmul fast float %t140, %il1712
%t1715 = fmul fast float %t1713, %t1079
%t1716 = fadd fast float %t1707, %t1715
  %il1721 = load float, float* @m4, align 4
%t1722 = fmul fast float %t176, %il1721
%t1724 = fmul fast float %t1722, %t1095
%t1725 = fsub fast float %t1716, %t1724
  ; let nvx3 = %t1725
  %ap_1727 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store float %t1137, ptr %ap_1727, align 4, !tbaa !3
  %fdp1730 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1729 = load float, ptr %fdp1730, align 4
  %il1733 = load float, float* @dt, align 4
%t1735 = fmul fast float %il1733, %t1137
%t1736 = fadd fast float %t1729, %t1735
  %ap_1737 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store float %t1736, ptr %ap_1737, align 4, !tbaa !3
  %fdp1740 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1739 = load float, ptr %fdp1740, align 4
  %il1743 = load float, float* @dt, align 4
%t1745 = fmul fast float %il1743, %t1179
%t1746 = fadd fast float %t1739, %t1745
  %ap_1747 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store float %t1746, ptr %ap_1747, align 4, !tbaa !3
  %ap_1749 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store float %t1179, ptr %ap_1749, align 4, !tbaa !3
  %ap_1751 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store float %t1221, ptr %ap_1751, align 4, !tbaa !3
  %fdp1754 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1753 = load float, ptr %fdp1754, align 4
  %il1757 = load float, float* @dt, align 4
%t1759 = fmul fast float %il1757, %t1221
%t1760 = fadd fast float %t1753, %t1759
  %ap_1761 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store float %t1760, ptr %ap_1761, align 4, !tbaa !3
  %ap_1763 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store float %t1263, ptr %ap_1763, align 4, !tbaa !3
  %fdp1766 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1765 = load float, ptr %fdp1766, align 4
  %il1769 = load float, float* @dt, align 4
%t1771 = fmul fast float %il1769, %t1263
%t1772 = fadd fast float %t1765, %t1771
  %ap_1773 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store float %t1772, ptr %ap_1773, align 4, !tbaa !3
  %ap_1775 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  store float %t1305, ptr %ap_1775, align 4, !tbaa !3
  %fdp1778 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1777 = load float, ptr %fdp1778, align 4
  %il1781 = load float, float* @dt, align 4
%t1783 = fmul fast float %il1781, %t1305
%t1784 = fadd fast float %t1777, %t1783
  %ap_1785 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store float %t1784, ptr %ap_1785, align 4, !tbaa !3
  %fdp1788 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1787 = load float, ptr %fdp1788, align 4
  %il1791 = load float, float* @dt, align 4
%t1793 = fmul fast float %il1791, %t1347
%t1794 = fadd fast float %t1787, %t1793
  %ap_1795 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store float %t1794, ptr %ap_1795, align 4, !tbaa !3
  %ap_1797 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  store float %t1347, ptr %ap_1797, align 4, !tbaa !3
  %fdp1800 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1799 = load float, ptr %fdp1800, align 4
  %il1803 = load float, float* @dt, align 4
%t1805 = fmul fast float %il1803, %t1389
%t1806 = fadd fast float %t1799, %t1805
  %ap_1807 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  store float %t1806, ptr %ap_1807, align 4, !tbaa !3
  %ap_1809 = getelementptr inbounds %State, ptr %state, i32 0, i32 18
  store float %t1389, ptr %ap_1809, align 4, !tbaa !3
  %fdp1812 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1811 = load float, ptr %fdp1812, align 4
  %il1815 = load float, float* @dt, align 4
%t1817 = fmul fast float %il1815, %t1431
%t1818 = fadd fast float %t1811, %t1817
  %ap_1819 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  store float %t1818, ptr %ap_1819, align 4, !tbaa !3
  %ap_1821 = getelementptr inbounds %State, ptr %state, i32 0, i32 19
  store float %t1431, ptr %ap_1821, align 4, !tbaa !3
  %ap_1823 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
  store float %t1473, ptr %ap_1823, align 4, !tbaa !3
  %fdp1826 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1825 = load float, ptr %fdp1826, align 4
  %il1829 = load float, float* @dt, align 4
%t1831 = fmul fast float %il1829, %t1473
%t1832 = fadd fast float %t1825, %t1831
  %ap_1833 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  store float %t1832, ptr %ap_1833, align 4, !tbaa !3
  %ap_1835 = getelementptr inbounds %State, ptr %state, i32 0, i32 30
  store float %t1515, ptr %ap_1835, align 4, !tbaa !3
  %fdp1838 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1837 = load float, ptr %fdp1838, align 4
  %il1841 = load float, float* @dt, align 4
%t1843 = fmul fast float %il1841, %t1515
%t1844 = fadd fast float %t1837, %t1843
  %ap_1845 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  store float %t1844, ptr %ap_1845, align 4, !tbaa !3
  %fdp1848 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1847 = load float, ptr %fdp1848, align 4
  %il1851 = load float, float* @dt, align 4
%t1853 = fmul fast float %il1851, %t1557
%t1854 = fadd fast float %t1847, %t1853
  %ap_1855 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  store float %t1854, ptr %ap_1855, align 4, !tbaa !3
  %ap_1857 = getelementptr inbounds %State, ptr %state, i32 0, i32 29
  store float %t1557, ptr %ap_1857, align 4, !tbaa !3
  %ap_1859 = getelementptr inbounds %State, ptr %state, i32 0, i32 31
  store float %t1599, ptr %ap_1859, align 4, !tbaa !3
  %fdp1862 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1861 = load float, ptr %fdp1862, align 4
  %il1865 = load float, float* @dt, align 4
%t1867 = fmul fast float %il1865, %t1599
%t1868 = fadd fast float %t1861, %t1867
  %ap_1869 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  store float %t1868, ptr %ap_1869, align 4, !tbaa !3
  %fdp1872 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1871 = load float, ptr %fdp1872, align 4
  %il1875 = load float, float* @dt, align 4
%t1877 = fmul fast float %il1875, %t1641
%t1878 = fadd fast float %t1871, %t1877
  %ap_1879 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  store float %t1878, ptr %ap_1879, align 4, !tbaa !3
  %ap_1881 = getelementptr inbounds %State, ptr %state, i32 0, i32 24
  store float %t1641, ptr %ap_1881, align 4, !tbaa !3
  %ap_1883 = getelementptr inbounds %State, ptr %state, i32 0, i32 25
  store float %t1683, ptr %ap_1883, align 4, !tbaa !3
  %fdp1886 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1885 = load float, ptr %fdp1886, align 4
  %il1889 = load float, float* @dt, align 4
%t1891 = fmul fast float %il1889, %t1683
%t1892 = fadd fast float %t1885, %t1891
  %ap_1893 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  store float %t1892, ptr %ap_1893, align 4, !tbaa !3
  %ap_1895 = getelementptr inbounds %State, ptr %state, i32 0, i32 23
  store float %t1725, ptr %ap_1895, align 4, !tbaa !3
  %fdp1898 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1897 = load float, ptr %fdp1898, align 4
  %il1901 = load float, float* @dt, align 4
%t1903 = fmul fast float %il1901, %t1725
%t1904 = fadd fast float %t1897, %t1903
  %ap_1905 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  store float %t1904, ptr %ap_1905, align 4, !tbaa !3
%ff1910 = bitcast i32 1056964608 to float
  %il1912 = load float, float* @m0, align 4
%t1913 = fmul fast float %ff1910, %il1912
  %fdp1918 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t1917 = load float, ptr %fdp1918, align 4
  %fdp1920 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t1919 = load float, ptr %fdp1920, align 4
%t1921 = fmul fast float %t1917, %t1919
  %fdp1924 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t1923 = load float, ptr %fdp1924, align 4
  %fdp1926 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t1925 = load float, ptr %fdp1926, align 4
%t1927 = fmul fast float %t1923, %t1925
%t1928 = fadd fast float %t1921, %t1927
  %fdp1931 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t1930 = load float, ptr %fdp1931, align 4
  %fdp1933 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t1932 = load float, ptr %fdp1933, align 4
%t1934 = fmul fast float %t1930, %t1932
%t1935 = fadd fast float %t1928, %t1934
%t1936 = fmul fast float %t1913, %t1935
  ; let ek0c = %t1936
  %fdp1939 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1938 = load float, ptr %fdp1939, align 4
  %fdp1941 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1940 = load float, ptr %fdp1941, align 4
%t1942 = fsub fast float %t1938, %t1940
  ; let dye01 = %t1942
  %fdp1945 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1944 = load float, ptr %fdp1945, align 4
  %fdp1947 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1946 = load float, ptr %fdp1947, align 4
%t1948 = fsub fast float %t1944, %t1946
  ; let dxe01 = %t1948
  %fdp1951 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1950 = load float, ptr %fdp1951, align 4
  %fdp1953 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1952 = load float, ptr %fdp1953, align 4
%t1954 = fsub fast float %t1950, %t1952
  ; let dze01 = %t1954
%ff1959 = bitcast i32 1056964608 to float
  %il1961 = load float, float* @m1, align 4
%t1962 = fmul fast float %ff1959, %il1961
  %fdp1967 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t1966 = load float, ptr %fdp1967, align 4
  %fdp1969 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t1968 = load float, ptr %fdp1969, align 4
%t1970 = fmul fast float %t1966, %t1968
  %fdp1973 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t1972 = load float, ptr %fdp1973, align 4
  %fdp1975 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t1974 = load float, ptr %fdp1975, align 4
%t1976 = fmul fast float %t1972, %t1974
%t1977 = fadd fast float %t1970, %t1976
  %fdp1980 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t1979 = load float, ptr %fdp1980, align 4
  %fdp1982 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t1981 = load float, ptr %fdp1982, align 4
%t1983 = fmul fast float %t1979, %t1981
%t1984 = fadd fast float %t1977, %t1983
%t1985 = fmul fast float %t1962, %t1984
  ; let ek1c = %t1985
  %fdp1988 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1987 = load float, ptr %fdp1988, align 4
  %fdp1990 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1989 = load float, ptr %fdp1990, align 4
%t1991 = fsub fast float %t1987, %t1989
  ; let dye12 = %t1991
  %fdp1994 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1993 = load float, ptr %fdp1994, align 4
  %fdp1996 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1995 = load float, ptr %fdp1996, align 4
%t1997 = fsub fast float %t1993, %t1995
  ; let dye02 = %t1997
  %fdp2000 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1999 = load float, ptr %fdp2000, align 4
  %fdp2002 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t2001 = load float, ptr %fdp2002, align 4
%t2003 = fsub fast float %t1999, %t2001
  ; let dze02 = %t2003
  %fdp2006 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t2005 = load float, ptr %fdp2006, align 4
  %fdp2008 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t2007 = load float, ptr %fdp2008, align 4
%t2009 = fsub fast float %t2005, %t2007
  ; let dze12 = %t2009
%ff2014 = bitcast i32 1056964608 to float
  %il2016 = load float, float* @m2, align 4
%t2017 = fmul fast float %ff2014, %il2016
  %fdp2022 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
  %t2021 = load float, ptr %fdp2022, align 4
  %fdp2024 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
  %t2023 = load float, ptr %fdp2024, align 4
%t2025 = fmul fast float %t2021, %t2023
  %fdp2028 = getelementptr inbounds %State, ptr %state, i32 0, i32 18
  %t2027 = load float, ptr %fdp2028, align 4
  %fdp2030 = getelementptr inbounds %State, ptr %state, i32 0, i32 18
  %t2029 = load float, ptr %fdp2030, align 4
%t2031 = fmul fast float %t2027, %t2029
%t2032 = fadd fast float %t2025, %t2031
  %fdp2035 = getelementptr inbounds %State, ptr %state, i32 0, i32 19
  %t2034 = load float, ptr %fdp2035, align 4
  %fdp2037 = getelementptr inbounds %State, ptr %state, i32 0, i32 19
  %t2036 = load float, ptr %fdp2037, align 4
%t2038 = fmul fast float %t2034, %t2036
%t2039 = fadd fast float %t2032, %t2038
%t2040 = fmul fast float %t2017, %t2039
  ; let ek2c = %t2040
  %fdp2043 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t2042 = load float, ptr %fdp2043, align 4
  %fdp2045 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t2044 = load float, ptr %fdp2045, align 4
%t2046 = fsub fast float %t2042, %t2044
  ; let dxe12 = %t2046
  %fdp2049 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t2048 = load float, ptr %fdp2049, align 4
  %fdp2051 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t2050 = load float, ptr %fdp2051, align 4
%t2052 = fsub fast float %t2048, %t2050
  ; let dxe02 = %t2052
  %fdp2055 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t2054 = load float, ptr %fdp2055, align 4
  %fdp2057 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t2056 = load float, ptr %fdp2057, align 4
%t2058 = fsub fast float %t2054, %t2056
  ; let dye24 = %t2058
  %fdp2061 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t2060 = load float, ptr %fdp2061, align 4
  %fdp2063 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t2062 = load float, ptr %fdp2063, align 4
%t2064 = fsub fast float %t2060, %t2062
  ; let dye04 = %t2064
  %fdp2067 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t2066 = load float, ptr %fdp2067, align 4
  %fdp2069 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t2068 = load float, ptr %fdp2069, align 4
%t2070 = fsub fast float %t2066, %t2068
  ; let dye14 = %t2070
  %fdp2073 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t2072 = load float, ptr %fdp2073, align 4
  %fdp2075 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t2074 = load float, ptr %fdp2075, align 4
%t2076 = fsub fast float %t2072, %t2074
  ; let dxe24 = %t2076
  %fdp2079 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t2078 = load float, ptr %fdp2079, align 4
  %fdp2081 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t2080 = load float, ptr %fdp2081, align 4
%t2082 = fsub fast float %t2078, %t2080
  ; let dxe14 = %t2082
  %fdp2085 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t2084 = load float, ptr %fdp2085, align 4
  %fdp2087 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t2086 = load float, ptr %fdp2087, align 4
%t2088 = fsub fast float %t2084, %t2086
  ; let dxe04 = %t2088
%ff2093 = bitcast i32 1056964608 to float
  %il2095 = load float, float* @m4, align 4
%t2096 = fmul fast float %ff2093, %il2095
  %fdp2101 = getelementptr inbounds %State, ptr %state, i32 0, i32 29
  %t2100 = load float, ptr %fdp2101, align 4
  %fdp2103 = getelementptr inbounds %State, ptr %state, i32 0, i32 29
  %t2102 = load float, ptr %fdp2103, align 4
%t2104 = fmul fast float %t2100, %t2102
  %fdp2107 = getelementptr inbounds %State, ptr %state, i32 0, i32 30
  %t2106 = load float, ptr %fdp2107, align 4
  %fdp2109 = getelementptr inbounds %State, ptr %state, i32 0, i32 30
  %t2108 = load float, ptr %fdp2109, align 4
%t2110 = fmul fast float %t2106, %t2108
%t2111 = fadd fast float %t2104, %t2110
  %fdp2114 = getelementptr inbounds %State, ptr %state, i32 0, i32 31
  %t2113 = load float, ptr %fdp2114, align 4
  %fdp2116 = getelementptr inbounds %State, ptr %state, i32 0, i32 31
  %t2115 = load float, ptr %fdp2116, align 4
%t2117 = fmul fast float %t2113, %t2115
%t2118 = fadd fast float %t2111, %t2117
%t2119 = fmul fast float %t2096, %t2118
  ; let ek4c = %t2119
  %fdp2122 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t2121 = load float, ptr %fdp2122, align 4
  %fdp2124 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t2123 = load float, ptr %fdp2124, align 4
%t2125 = fsub fast float %t2121, %t2123
  ; let dze04 = %t2125
  %fdp2128 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t2127 = load float, ptr %fdp2128, align 4
  %fdp2130 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t2129 = load float, ptr %fdp2130, align 4
%t2131 = fsub fast float %t2127, %t2129
  ; let dze24 = %t2131
  %fdp2134 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t2133 = load float, ptr %fdp2134, align 4
  %fdp2136 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t2135 = load float, ptr %fdp2136, align 4
%t2137 = fsub fast float %t2133, %t2135
  ; let dze14 = %t2137
  %fdp2140 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t2139 = load float, ptr %fdp2140, align 4
  %fdp2142 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t2141 = load float, ptr %fdp2142, align 4
%t2143 = fsub fast float %t2139, %t2141
  ; let dye13 = %t2143
  %fdp2146 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t2145 = load float, ptr %fdp2146, align 4
  %fdp2148 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t2147 = load float, ptr %fdp2148, align 4
%t2149 = fsub fast float %t2145, %t2147
  ; let dye34 = %t2149
  %fdp2152 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t2151 = load float, ptr %fdp2152, align 4
  %fdp2154 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t2153 = load float, ptr %fdp2154, align 4
%t2155 = fsub fast float %t2151, %t2153
  ; let dye03 = %t2155
  %fdp2158 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t2157 = load float, ptr %fdp2158, align 4
  %fdp2160 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t2159 = load float, ptr %fdp2160, align 4
%t2161 = fsub fast float %t2157, %t2159
  ; let dye23 = %t2161
  %fdp2164 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t2163 = load float, ptr %fdp2164, align 4
  %fdp2166 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t2165 = load float, ptr %fdp2166, align 4
%t2167 = fsub fast float %t2163, %t2165
  ; let dze34 = %t2167
  %fdp2170 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t2169 = load float, ptr %fdp2170, align 4
  %fdp2172 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t2171 = load float, ptr %fdp2172, align 4
%t2173 = fsub fast float %t2169, %t2171
  ; let dze03 = %t2173
  %fdp2176 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t2175 = load float, ptr %fdp2176, align 4
  %fdp2178 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t2177 = load float, ptr %fdp2178, align 4
%t2179 = fsub fast float %t2175, %t2177
  ; let dze13 = %t2179
  %fdp2182 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t2181 = load float, ptr %fdp2182, align 4
  %fdp2184 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t2183 = load float, ptr %fdp2184, align 4
%t2185 = fsub fast float %t2181, %t2183
  ; let dze23 = %t2185
%ff2190 = bitcast i32 1056964608 to float
  %il2192 = load float, float* @m3, align 4
%t2193 = fmul fast float %ff2190, %il2192
  %fdp2198 = getelementptr inbounds %State, ptr %state, i32 0, i32 23
  %t2197 = load float, ptr %fdp2198, align 4
  %fdp2200 = getelementptr inbounds %State, ptr %state, i32 0, i32 23
  %t2199 = load float, ptr %fdp2200, align 4
%t2201 = fmul fast float %t2197, %t2199
  %fdp2204 = getelementptr inbounds %State, ptr %state, i32 0, i32 24
  %t2203 = load float, ptr %fdp2204, align 4
  %fdp2206 = getelementptr inbounds %State, ptr %state, i32 0, i32 24
  %t2205 = load float, ptr %fdp2206, align 4
%t2207 = fmul fast float %t2203, %t2205
%t2208 = fadd fast float %t2201, %t2207
  %fdp2211 = getelementptr inbounds %State, ptr %state, i32 0, i32 25
  %t2210 = load float, ptr %fdp2211, align 4
  %fdp2213 = getelementptr inbounds %State, ptr %state, i32 0, i32 25
  %t2212 = load float, ptr %fdp2213, align 4
%t2214 = fmul fast float %t2210, %t2212
%t2215 = fadd fast float %t2208, %t2214
%t2216 = fmul fast float %t2193, %t2215
  ; let ek3c = %t2216
  %fdp2219 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t2218 = load float, ptr %fdp2219, align 4
  %fdp2221 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t2220 = load float, ptr %fdp2221, align 4
%t2222 = fsub fast float %t2218, %t2220
  ; let dxe34 = %t2222
  %fdp2225 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t2224 = load float, ptr %fdp2225, align 4
  %fdp2227 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t2226 = load float, ptr %fdp2227, align 4
%t2228 = fsub fast float %t2224, %t2226
  ; let dxe23 = %t2228
  %fdp2231 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t2230 = load float, ptr %fdp2231, align 4
  %fdp2233 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t2232 = load float, ptr %fdp2233, align 4
%t2234 = fsub fast float %t2230, %t2232
  ; let dxe13 = %t2234
  %fdp2237 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t2236 = load float, ptr %fdp2237, align 4
  %fdp2239 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t2238 = load float, ptr %fdp2239, align 4
%t2240 = fsub fast float %t2236, %t2238
  ; let dxe03 = %t2240
%t2246 = fmul fast float %t1948, %t1948
%t2250 = fmul fast float %t1942, %t1942
%t2251 = fadd fast float %t2246, %t2250
%t2255 = fmul fast float %t1954, %t1954
%t2256 = fadd fast float %t2251, %t2255
  ; let dsqe01 = %t2256
%t2262 = fmul fast float %t2046, %t2046
%t2266 = fmul fast float %t1991, %t1991
%t2267 = fadd fast float %t2262, %t2266
%t2271 = fmul fast float %t2009, %t2009
%t2272 = fadd fast float %t2267, %t2271
  ; let dsqe12 = %t2272
%t2278 = fmul fast float %t2052, %t2052
%t2282 = fmul fast float %t1997, %t1997
%t2283 = fadd fast float %t2278, %t2282
%t2287 = fmul fast float %t2003, %t2003
%t2288 = fadd fast float %t2283, %t2287
  ; let dsqe02 = %t2288
%t2294 = fmul fast float %t2088, %t2088
%t2298 = fmul fast float %t2064, %t2064
%t2299 = fadd fast float %t2294, %t2298
%t2303 = fmul fast float %t2125, %t2125
%t2304 = fadd fast float %t2299, %t2303
  ; let dsqe04 = %t2304
%t2310 = fmul fast float %t2076, %t2076
%t2314 = fmul fast float %t2058, %t2058
%t2315 = fadd fast float %t2310, %t2314
%t2319 = fmul fast float %t2131, %t2131
%t2320 = fadd fast float %t2315, %t2319
  ; let dsqe24 = %t2320
%t2326 = fmul fast float %t2082, %t2082
%t2330 = fmul fast float %t2070, %t2070
%t2331 = fadd fast float %t2326, %t2330
%t2335 = fmul fast float %t2137, %t2137
%t2336 = fadd fast float %t2331, %t2335
  ; let dsqe14 = %t2336
%t2343 = fadd fast float %t1936, %t1985
%t2345 = fadd fast float %t2343, %t2040
%t2347 = fadd fast float %t2345, %t2216
%t2349 = fadd fast float %t2347, %t2119
  ; let ekc = %t2349
%t2355 = fmul fast float %t2222, %t2222
%t2359 = fmul fast float %t2149, %t2149
%t2360 = fadd fast float %t2355, %t2359
%t2364 = fmul fast float %t2167, %t2167
%t2365 = fadd fast float %t2360, %t2364
  ; let dsqe34 = %t2365
%t2371 = fmul fast float %t2228, %t2228
%t2375 = fmul fast float %t2161, %t2161
%t2376 = fadd fast float %t2371, %t2375
%t2380 = fmul fast float %t2185, %t2185
%t2381 = fadd fast float %t2376, %t2380
  ; let dsqe23 = %t2381
%t2387 = fmul fast float %t2234, %t2234
%t2391 = fmul fast float %t2143, %t2143
%t2392 = fadd fast float %t2387, %t2391
%t2396 = fmul fast float %t2179, %t2179
%t2397 = fadd fast float %t2392, %t2396
  ; let dsqe13 = %t2397
%t2403 = fmul fast float %t2240, %t2240
%t2407 = fmul fast float %t2155, %t2155
%t2408 = fadd fast float %t2403, %t2407
%t2412 = fmul fast float %t2173, %t2173
%t2413 = fadd fast float %t2408, %t2412
  ; let dsqe03 = %t2413
%ff2418 = bitcast i32 1056964608 to float
%t2419 = fmul fast float %t2256, %ff2418
  ; let gea01 = %t2419
%ff2424 = bitcast i32 1056964608 to float
%t2425 = fmul fast float %t2272, %ff2424
  ; let gea12 = %t2425
%ff2430 = bitcast i32 1056964608 to float
%t2431 = fmul fast float %t2288, %ff2430
  ; let gea02 = %t2431
%ff2436 = bitcast i32 1056964608 to float
%t2437 = fmul fast float %t2304, %ff2436
  ; let gea04 = %t2437
%ff2442 = bitcast i32 1056964608 to float
%t2443 = fmul fast float %t2320, %ff2442
  ; let gea24 = %t2443
%ff2448 = bitcast i32 1056964608 to float
%t2449 = fmul fast float %t2336, %ff2448
  ; let gea14 = %t2449
%ff2454 = bitcast i32 1056964608 to float
%t2455 = fmul fast float %t2365, %ff2454
  ; let gea34 = %t2455
%ff2460 = bitcast i32 1056964608 to float
%t2461 = fmul fast float %t2381, %ff2460
  ; let gea23 = %t2461
%ff2466 = bitcast i32 1056964608 to float
%t2467 = fmul fast float %t2397, %ff2466
  ; let gea13 = %t2467
%ff2472 = bitcast i32 1056964608 to float
%t2473 = fmul fast float %t2413, %ff2472
  ; let gea03 = %t2473
%ff2477 = bitcast i32 1056964608 to float
%t2483 = fdiv fast float %t2256, %t2419
%t2484 = fadd fast float %t2419, %t2483
%t2485 = fmul fast float %ff2477, %t2484
  ; let geb01 = %t2485
%ff2489 = bitcast i32 1056964608 to float
%t2495 = fdiv fast float %t2272, %t2425
%t2496 = fadd fast float %t2425, %t2495
%t2497 = fmul fast float %ff2489, %t2496
  ; let geb12 = %t2497
%ff2501 = bitcast i32 1056964608 to float
%t2507 = fdiv fast float %t2288, %t2431
%t2508 = fadd fast float %t2431, %t2507
%t2509 = fmul fast float %ff2501, %t2508
  ; let geb02 = %t2509
%ff2513 = bitcast i32 1056964608 to float
%t2519 = fdiv fast float %t2304, %t2437
%t2520 = fadd fast float %t2437, %t2519
%t2521 = fmul fast float %ff2513, %t2520
  ; let geb04 = %t2521
%ff2525 = bitcast i32 1056964608 to float
%t2531 = fdiv fast float %t2320, %t2443
%t2532 = fadd fast float %t2443, %t2531
%t2533 = fmul fast float %ff2525, %t2532
  ; let geb24 = %t2533
%ff2537 = bitcast i32 1056964608 to float
%t2543 = fdiv fast float %t2336, %t2449
%t2544 = fadd fast float %t2449, %t2543
%t2545 = fmul fast float %ff2537, %t2544
  ; let geb14 = %t2545
%ff2549 = bitcast i32 1056964608 to float
%t2555 = fdiv fast float %t2365, %t2455
%t2556 = fadd fast float %t2455, %t2555
%t2557 = fmul fast float %ff2549, %t2556
  ; let geb34 = %t2557
%ff2561 = bitcast i32 1056964608 to float
%t2567 = fdiv fast float %t2381, %t2461
%t2568 = fadd fast float %t2461, %t2567
%t2569 = fmul fast float %ff2561, %t2568
  ; let geb23 = %t2569
%ff2573 = bitcast i32 1056964608 to float
%t2579 = fdiv fast float %t2397, %t2467
%t2580 = fadd fast float %t2467, %t2579
%t2581 = fmul fast float %ff2573, %t2580
  ; let geb13 = %t2581
%ff2585 = bitcast i32 1056964608 to float
%t2591 = fdiv fast float %t2413, %t2473
%t2592 = fadd fast float %t2473, %t2591
%t2593 = fmul fast float %ff2585, %t2592
  ; let geb03 = %t2593
%ff2597 = bitcast i32 1056964608 to float
%t2603 = fdiv fast float %t2256, %t2485
%t2604 = fadd fast float %t2485, %t2603
%t2605 = fmul fast float %ff2597, %t2604
  ; let gec01 = %t2605
%ff2609 = bitcast i32 1056964608 to float
%t2615 = fdiv fast float %t2272, %t2497
%t2616 = fadd fast float %t2497, %t2615
%t2617 = fmul fast float %ff2609, %t2616
  ; let gec12 = %t2617
%ff2621 = bitcast i32 1056964608 to float
%t2627 = fdiv fast float %t2288, %t2509
%t2628 = fadd fast float %t2509, %t2627
%t2629 = fmul fast float %ff2621, %t2628
  ; let gec02 = %t2629
%ff2633 = bitcast i32 1056964608 to float
%t2639 = fdiv fast float %t2304, %t2521
%t2640 = fadd fast float %t2521, %t2639
%t2641 = fmul fast float %ff2633, %t2640
  ; let gec04 = %t2641
%ff2645 = bitcast i32 1056964608 to float
%t2651 = fdiv fast float %t2320, %t2533
%t2652 = fadd fast float %t2533, %t2651
%t2653 = fmul fast float %ff2645, %t2652
  ; let gec24 = %t2653
%ff2657 = bitcast i32 1056964608 to float
%t2663 = fdiv fast float %t2336, %t2545
%t2664 = fadd fast float %t2545, %t2663
%t2665 = fmul fast float %ff2657, %t2664
  ; let gec14 = %t2665
%ff2669 = bitcast i32 1056964608 to float
%t2675 = fdiv fast float %t2365, %t2557
%t2676 = fadd fast float %t2557, %t2675
%t2677 = fmul fast float %ff2669, %t2676
  ; let gec34 = %t2677
%ff2681 = bitcast i32 1056964608 to float
%t2687 = fdiv fast float %t2381, %t2569
%t2688 = fadd fast float %t2569, %t2687
%t2689 = fmul fast float %ff2681, %t2688
  ; let gec23 = %t2689
%ff2693 = bitcast i32 1056964608 to float
%t2699 = fdiv fast float %t2397, %t2581
%t2700 = fadd fast float %t2581, %t2699
%t2701 = fmul fast float %ff2693, %t2700
  ; let gec13 = %t2701
%ff2705 = bitcast i32 1056964608 to float
%t2711 = fdiv fast float %t2413, %t2593
%t2712 = fadd fast float %t2593, %t2711
%t2713 = fmul fast float %ff2705, %t2712
  ; let gec03 = %t2713
%ff2717 = bitcast i32 1056964608 to float
%t2723 = fdiv fast float %t2256, %t2605
%t2724 = fadd fast float %t2605, %t2723
%t2725 = fmul fast float %ff2717, %t2724
  ; let ged01 = %t2725
%ff2729 = bitcast i32 1056964608 to float
%t2735 = fdiv fast float %t2272, %t2617
%t2736 = fadd fast float %t2617, %t2735
%t2737 = fmul fast float %ff2729, %t2736
  ; let ged12 = %t2737
%ff2741 = bitcast i32 1056964608 to float
%t2747 = fdiv fast float %t2288, %t2629
%t2748 = fadd fast float %t2629, %t2747
%t2749 = fmul fast float %ff2741, %t2748
  ; let ged02 = %t2749
%ff2753 = bitcast i32 1056964608 to float
%t2759 = fdiv fast float %t2304, %t2641
%t2760 = fadd fast float %t2641, %t2759
%t2761 = fmul fast float %ff2753, %t2760
  ; let ged04 = %t2761
%ff2765 = bitcast i32 1056964608 to float
%t2771 = fdiv fast float %t2320, %t2653
%t2772 = fadd fast float %t2653, %t2771
%t2773 = fmul fast float %ff2765, %t2772
  ; let ged24 = %t2773
%ff2777 = bitcast i32 1056964608 to float
%t2783 = fdiv fast float %t2336, %t2665
%t2784 = fadd fast float %t2665, %t2783
%t2785 = fmul fast float %ff2777, %t2784
  ; let ged14 = %t2785
%ff2789 = bitcast i32 1056964608 to float
%t2795 = fdiv fast float %t2365, %t2677
%t2796 = fadd fast float %t2677, %t2795
%t2797 = fmul fast float %ff2789, %t2796
  ; let ged34 = %t2797
%ff2801 = bitcast i32 1056964608 to float
%t2807 = fdiv fast float %t2381, %t2689
%t2808 = fadd fast float %t2689, %t2807
%t2809 = fmul fast float %ff2801, %t2808
  ; let ged23 = %t2809
%ff2813 = bitcast i32 1056964608 to float
%t2819 = fdiv fast float %t2397, %t2701
%t2820 = fadd fast float %t2701, %t2819
%t2821 = fmul fast float %ff2813, %t2820
  ; let ged13 = %t2821
%ff2825 = bitcast i32 1056964608 to float
%t2831 = fdiv fast float %t2413, %t2713
%t2832 = fadd fast float %t2713, %t2831
%t2833 = fmul fast float %ff2825, %t2832
  ; let ged03 = %t2833
%ff2837 = bitcast i32 1056964608 to float
%t2843 = fdiv fast float %t2256, %t2725
%t2844 = fadd fast float %t2725, %t2843
%t2845 = fmul fast float %ff2837, %t2844
  ; let gee01 = %t2845
%ff2849 = bitcast i32 1056964608 to float
%t2855 = fdiv fast float %t2272, %t2737
%t2856 = fadd fast float %t2737, %t2855
%t2857 = fmul fast float %ff2849, %t2856
  ; let gee12 = %t2857
%ff2861 = bitcast i32 1056964608 to float
%t2867 = fdiv fast float %t2288, %t2749
%t2868 = fadd fast float %t2749, %t2867
%t2869 = fmul fast float %ff2861, %t2868
  ; let gee02 = %t2869
%ff2873 = bitcast i32 1056964608 to float
%t2879 = fdiv fast float %t2304, %t2761
%t2880 = fadd fast float %t2761, %t2879
%t2881 = fmul fast float %ff2873, %t2880
  ; let gee04 = %t2881
%ff2885 = bitcast i32 1056964608 to float
%t2891 = fdiv fast float %t2320, %t2773
%t2892 = fadd fast float %t2773, %t2891
%t2893 = fmul fast float %ff2885, %t2892
  ; let gee24 = %t2893
%ff2897 = bitcast i32 1056964608 to float
%t2903 = fdiv fast float %t2336, %t2785
%t2904 = fadd fast float %t2785, %t2903
%t2905 = fmul fast float %ff2897, %t2904
  ; let gee14 = %t2905
%ff2909 = bitcast i32 1056964608 to float
%t2915 = fdiv fast float %t2365, %t2797
%t2916 = fadd fast float %t2797, %t2915
%t2917 = fmul fast float %ff2909, %t2916
  ; let gee34 = %t2917
%ff2921 = bitcast i32 1056964608 to float
%t2927 = fdiv fast float %t2381, %t2809
%t2928 = fadd fast float %t2809, %t2927
%t2929 = fmul fast float %ff2921, %t2928
  ; let gee23 = %t2929
%ff2933 = bitcast i32 1056964608 to float
%t2939 = fdiv fast float %t2397, %t2821
%t2940 = fadd fast float %t2821, %t2939
%t2941 = fmul fast float %ff2933, %t2940
  ; let gee13 = %t2941
%ff2945 = bitcast i32 1056964608 to float
%t2951 = fdiv fast float %t2413, %t2833
%t2952 = fadd fast float %t2833, %t2951
%t2953 = fmul fast float %ff2945, %t2952
  ; let gee03 = %t2953
%ff2957 = bitcast i32 1056964608 to float
%t2963 = fdiv fast float %t2256, %t2845
%t2964 = fadd fast float %t2845, %t2963
%t2965 = fmul fast float %ff2957, %t2964
  ; let edist01 = %t2965
%ff2969 = bitcast i32 1056964608 to float
%t2975 = fdiv fast float %t2272, %t2857
%t2976 = fadd fast float %t2857, %t2975
%t2977 = fmul fast float %ff2969, %t2976
  ; let edist12 = %t2977
%ff2981 = bitcast i32 1056964608 to float
%t2987 = fdiv fast float %t2288, %t2869
%t2988 = fadd fast float %t2869, %t2987
%t2989 = fmul fast float %ff2981, %t2988
  ; let edist02 = %t2989
%ff2993 = bitcast i32 1056964608 to float
%t2999 = fdiv fast float %t2304, %t2881
%t3000 = fadd fast float %t2881, %t2999
%t3001 = fmul fast float %ff2993, %t3000
  ; let edist04 = %t3001
%ff3005 = bitcast i32 1056964608 to float
%t3011 = fdiv fast float %t2320, %t2893
%t3012 = fadd fast float %t2893, %t3011
%t3013 = fmul fast float %ff3005, %t3012
  ; let edist24 = %t3013
%ff3017 = bitcast i32 1056964608 to float
%t3023 = fdiv fast float %t2336, %t2905
%t3024 = fadd fast float %t2905, %t3023
%t3025 = fmul fast float %ff3017, %t3024
  ; let edist14 = %t3025
%ff3029 = bitcast i32 1056964608 to float
%t3035 = fdiv fast float %t2365, %t2917
%t3036 = fadd fast float %t2917, %t3035
%t3037 = fmul fast float %ff3029, %t3036
  ; let edist34 = %t3037
%ff3041 = bitcast i32 1056964608 to float
%t3047 = fdiv fast float %t2381, %t2929
%t3048 = fadd fast float %t2929, %t3047
%t3049 = fmul fast float %ff3041, %t3048
  ; let edist23 = %t3049
%ff3053 = bitcast i32 1056964608 to float
%t3059 = fdiv fast float %t2397, %t2941
%t3060 = fadd fast float %t2941, %t3059
%t3061 = fmul fast float %ff3053, %t3060
  ; let edist13 = %t3061
%ff3065 = bitcast i32 1056964608 to float
%t3071 = fdiv fast float %t2413, %t2953
%t3072 = fadd fast float %t2953, %t3071
%t3073 = fmul fast float %ff3065, %t3072
  ; let edist03 = %t3073
  %il3077 = load float, float* @m0, align 4
  %il3079 = load float, float* @m1, align 4
%t3080 = fmul fast float %il3077, %il3079
%t3082 = fdiv fast float %t3080, %t2965
  ; let epex01 = %t3082
  %il3086 = load float, float* @m1, align 4
  %il3088 = load float, float* @m2, align 4
%t3089 = fmul fast float %il3086, %il3088
%t3091 = fdiv fast float %t3089, %t2977
  ; let epex12 = %t3091
  %il3095 = load float, float* @m0, align 4
  %il3097 = load float, float* @m2, align 4
%t3098 = fmul fast float %il3095, %il3097
%t3100 = fdiv fast float %t3098, %t2989
  ; let epex02 = %t3100
  %il3104 = load float, float* @m0, align 4
  %il3106 = load float, float* @m4, align 4
%t3107 = fmul fast float %il3104, %il3106
%t3109 = fdiv fast float %t3107, %t3001
  ; let epex04 = %t3109
  %il3113 = load float, float* @m2, align 4
  %il3115 = load float, float* @m4, align 4
%t3116 = fmul fast float %il3113, %il3115
%t3118 = fdiv fast float %t3116, %t3013
  ; let epex24 = %t3118
  %il3122 = load float, float* @m1, align 4
  %il3124 = load float, float* @m4, align 4
%t3125 = fmul fast float %il3122, %il3124
%t3127 = fdiv fast float %t3125, %t3025
  ; let epex14 = %t3127
  %il3131 = load float, float* @m3, align 4
  %il3133 = load float, float* @m4, align 4
%t3134 = fmul fast float %il3131, %il3133
%t3136 = fdiv fast float %t3134, %t3037
  ; let epex34 = %t3136
  %il3140 = load float, float* @m2, align 4
  %il3142 = load float, float* @m3, align 4
%t3143 = fmul fast float %il3140, %il3142
%t3145 = fdiv fast float %t3143, %t3049
  ; let epex23 = %t3145
  %il3149 = load float, float* @m1, align 4
  %il3151 = load float, float* @m3, align 4
%t3152 = fmul fast float %il3149, %il3151
%t3154 = fdiv fast float %t3152, %t3061
  ; let epex13 = %t3154
  %il3158 = load float, float* @m0, align 4
  %il3160 = load float, float* @m3, align 4
%t3161 = fmul fast float %il3158, %il3160
%t3163 = fdiv fast float %t3161, %t3073
  ; let epex03 = %t3163
%t3177 = fadd fast float %t3082, %t3100
%t3179 = fadd fast float %t3177, %t3163
%t3181 = fadd fast float %t3179, %t3109
%t3183 = fadd fast float %t3181, %t3091
%t3185 = fadd fast float %t3183, %t3154
%t3187 = fadd fast float %t3185, %t3127
%t3189 = fadd fast float %t3187, %t3145
%t3191 = fadd fast float %t3189, %t3118
%t3193 = fadd fast float %t3191, %t3136
  %t3194 = fneg float %t3193
  ; let epp = %t3194
%t3198 = fadd fast float %t3194, %t2349
  ; let energy = %t3198
  %ap_3200 = getelementptr inbounds %State, ptr %state, i32 0, i32 32
  store float %t3198, ptr %ap_3200, align 4, !tbaa !3
  %fdp3204 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t3203 = load i64, i64* %fdp3204, align 8
%t3206 = add i64 0, 5000000
%t3207 = srem i64 %t3203, %t3206
%t3209 = add i64 0, 0
%c3210 = icmp eq i64 %t3207, %t3209
  br i1 %c3210, label %g3211_t, label %g3211_e
  g3211_t:
    %pfd3214 = fpext float %t3198 to double
    %pso3215 = load volatile ptr, ptr @stdout
    %pff3216 = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0
    %ppf3217 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso3215, ptr %pff3216, double %pfd3214)
    %t3212 = zext i32 %ppf3217 to i64
    ; let __periodic = %t3212
    br label %g3211_tx
  g3211_tx:
    br label %g3211_e
  g3211_e:
  %fdp3220 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t3219 = load i64, i64* %fdp3220, align 8
  %fdp3222 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3221 = load i64, i64* %fdp3222, align 8
%c3223 = icmp eq i64 %t3219, %t3221
  br i1 %c3223, label %g3224_t, label %g3224_e
  g3224_t:
    %fdp3227 = getelementptr inbounds %State, ptr %state, i32 0, i32 32
    %t3226 = load float, ptr %fdp3227, align 4
    %pfd3228 = fpext float %t3226 to double
    %pso3229 = load volatile ptr, ptr @stdout
    %pff3230 = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0
    %ppf3231 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso3229, ptr %pff3230, double %pfd3228)
    %t3225 = zext i32 %ppf3231 to i64
    ret void
  g3224_e:
  ret void
}

define internal i1 @pre_simulate(ptr noalias nocapture align 8 %state) #10 {
  entry:
  %fdp2 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t1 = load i64, i64* %fdp2, align 8
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 8
%c5 = icmp slt i64 %t1, %t3
  ret i1 %c5
}
define void @init_state(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 {
  entry:
  %ip_0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
%sp3 = getelementptr inbounds [6 x i8], [6 x i8]* @str.3, i64 0, i64 0
%t2 = ptrtoint i8* %sp3 to i64
  %gsr4 = inttoptr i64 %t2 to ptr
  %gsp5 = bitcast ptr %gsr4 to ptr
  %gdp6 = load i64, ptr %gsp5, align 8
  %gnp7 = inttoptr i64 %gdp6 to ptr
  %gnv8 = call ptr @getenv(ptr %gnp7)
  %gnvl9 = icmp eq ptr %gnv8, null
  br i1 %gnvl9, label %genv_nul10, label %genv_ok11
  genv_nul10:
    br label %genv_af12
  genv_ok11:
  %gav13 = call i64 @atol(ptr %gnv8)
    br label %genv_af12
  genv_af12:
  %t0 = phi i64 [ 0, %genv_nul10 ], [ %gav13, %genv_ok11 ]
  store i64 %t0, ptr %ip_0, align 8
  %ip_1 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
%t15 = add i64 0, 0
  store i64 %t15, ptr %ip_1, align 8
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
  %ip_8b = bitcast i32 1083895042 to float
  store float %ip_8b, ptr %ip_8, align 4
  %ip_9 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
%ff20 = bitcast i32 1066698078 to float
  %t21 = fneg float %ff20
  %rbi22 = bitcast float %t21 to i32
  %rze23 = zext i32 %rbi22 to i64
  %uf25 = trunc i64 %rze23 to i32
  %ub24 = bitcast i32 %uf25 to float
  store float %ub24, ptr %ip_9, align 4
  %ip_10 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
%ff30 = bitcast i32 1037318091 to float
  %t31 = fneg float %ff30
  %rbi32 = bitcast float %t31 to i32
  %rze33 = zext i32 %rbi32 to i64
  %uf35 = trunc i64 %rze33 to i32
  %ub34 = bitcast i32 %uf35 to float
  store float %ub34, ptr %ip_10, align 4
  %ip_11 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
%ff39 = bitcast i32 987338478 to float
  %il41 = load float, float* @dpy, align 4
%t42 = fmul fast float %ff39, %il41
  %rbi43 = bitcast float %t42 to i32
  %rze44 = zext i32 %rbi43 to i64
  %uf46 = trunc i64 %rze44 to i32
  %ub45 = bitcast i32 %uf46 to float
  store float %ub45, ptr %ip_11, align 4
  %ip_12 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
%ff50 = bitcast i32 1006389245 to float
  %il52 = load float, float* @dpy, align 4
%t53 = fmul fast float %ff50, %il52
  %rbi54 = bitcast float %t53 to i32
  %rze55 = zext i32 %rbi54 to i64
  %uf57 = trunc i64 %rze55 to i32
  %ub56 = bitcast i32 %uf57 to float
  store float %ub56, ptr %ip_12, align 4
  %ip_13 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
%ff63 = bitcast i32 949013706 to float
  %t64 = fneg float %ff63
  %il66 = load float, float* @dpy, align 4
%t67 = fmul fast float %t64, %il66
  %rbi68 = bitcast float %t67 to i32
  %rze69 = zext i32 %rbi68 to i64
  %uf71 = trunc i64 %rze69 to i32
  %ub70 = bitcast i32 %uf71 to float
  store float %ub70, ptr %ip_13, align 4
  %ip_14 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %ip_14b = bitcast i32 1090879086 to float
  store float %ip_14b, ptr %ip_14, align 4
  %ip_15 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %ip_15b = bitcast i32 1082392154 to float
  store float %ip_15b, ptr %ip_15, align 4
  %ip_16 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
%ff76 = bitcast i32 1053727391 to float
  %t77 = fneg float %ff76
  %rbi78 = bitcast float %t77 to i32
  %rze79 = zext i32 %rbi78 to i64
  %uf81 = trunc i64 %rze79 to i32
  %ub80 = bitcast i32 %uf81 to float
  store float %ub80, ptr %ip_16, align 4
  %ip_17 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
%ff87 = bitcast i32 993353136 to float
  %t88 = fneg float %ff87
  %il90 = load float, float* @dpy, align 4
%t91 = fmul fast float %t88, %il90
  %rbi92 = bitcast float %t91 to i32
  %rze93 = zext i32 %rbi92 to i64
  %uf95 = trunc i64 %rze93 to i32
  %ub94 = bitcast i32 %uf95 to float
  store float %ub94, ptr %ip_17, align 4
  %ip_18 = getelementptr inbounds %State, ptr %state, i32 0, i32 18
%ff99 = bitcast i32 1000590001 to float
  %il101 = load float, float* @dpy, align 4
%t102 = fmul fast float %ff99, %il101
  %rbi103 = bitcast float %t102 to i32
  %rze104 = zext i32 %rbi103 to i64
  %uf106 = trunc i64 %rze104 to i32
  %ub105 = bitcast i32 %uf106 to float
  store float %ub105, ptr %ip_18, align 4
  %ip_19 = getelementptr inbounds %State, ptr %state, i32 0, i32 19
%ff110 = bitcast i32 935414205 to float
  %il112 = load float, float* @dpy, align 4
%t113 = fmul fast float %ff110, %il112
  %rbi114 = bitcast float %t113 to i32
  %rze115 = zext i32 %rbi114 to i64
  %uf117 = trunc i64 %rze115 to i32
  %ub116 = bitcast i32 %uf117 to float
  store float %ub116, ptr %ip_19, align 4
  %ip_20 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %ip_20b = bitcast i32 1095651158 to float
  store float %ip_20b, ptr %ip_20, align 4
  %ip_21 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
%ff122 = bitcast i32 1097975623 to float
  %t123 = fneg float %ff122
  %rbi124 = bitcast float %t123 to i32
  %rze125 = zext i32 %rbi124 to i64
  %uf127 = trunc i64 %rze125 to i32
  %ub126 = bitcast i32 %uf127 to float
  store float %ub126, ptr %ip_21, align 4
  %ip_22 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
%ff132 = bitcast i32 1046784702 to float
  %t133 = fneg float %ff132
  %rbi134 = bitcast float %t133 to i32
  %rze135 = zext i32 %rbi134 to i64
  %uf137 = trunc i64 %rze135 to i32
  %ub136 = bitcast i32 %uf137 to float
  store float %ub136, ptr %ip_22, align 4
  %ip_23 = getelementptr inbounds %State, ptr %state, i32 0, i32 23
%ff141 = bitcast i32 994200002 to float
  %il143 = load float, float* @dpy, align 4
%t144 = fmul fast float %ff141, %il143
  %rbi145 = bitcast float %t144 to i32
  %rze146 = zext i32 %rbi145 to i64
  %uf148 = trunc i64 %rze146 to i32
  %ub147 = bitcast i32 %uf148 to float
  store float %ub147, ptr %ip_23, align 4
  %ip_24 = getelementptr inbounds %State, ptr %state, i32 0, i32 24
%ff152 = bitcast i32 991682594 to float
  %il154 = load float, float* @dpy, align 4
%t155 = fmul fast float %ff152, %il154
  %rbi156 = bitcast float %t155 to i32
  %rze157 = zext i32 %rbi156 to i64
  %uf159 = trunc i64 %rze157 to i32
  %ub158 = bitcast i32 %uf159 to float
  store float %ub158, ptr %ip_24, align 4
  %ip_25 = getelementptr inbounds %State, ptr %state, i32 0, i32 25
%ff165 = bitcast i32 939052064 to float
  %t166 = fneg float %ff165
  %il168 = load float, float* @dpy, align 4
%t169 = fmul fast float %t166, %il168
  %rbi170 = bitcast float %t169 to i32
  %rze171 = zext i32 %rbi170 to i64
  %uf173 = trunc i64 %rze171 to i32
  %ub172 = bitcast i32 %uf173 to float
  store float %ub172, ptr %ip_25, align 4
  %ip_26 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %ip_26b = bitcast i32 1098257213 to float
  store float %ip_26b, ptr %ip_26, align 4
  %ip_27 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
%ff178 = bitcast i32 1104108226 to float
  %t179 = fneg float %ff178
  %rbi180 = bitcast float %t179 to i32
  %rze181 = zext i32 %rbi180 to i64
  %uf183 = trunc i64 %rze181 to i32
  %ub182 = bitcast i32 %uf183 to float
  store float %ub182, ptr %ip_27, align 4
  %ip_28 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %ip_28b = bitcast i32 1043828637 to float
  store float %ip_28b, ptr %ip_28, align 4
  %ip_29 = getelementptr inbounds %State, ptr %state, i32 0, i32 29
%ff187 = bitcast i32 992980559 to float
  %il189 = load float, float* @dpy, align 4
%t190 = fmul fast float %ff187, %il189
  %rbi191 = bitcast float %t190 to i32
  %rze192 = zext i32 %rbi191 to i64
  %uf194 = trunc i64 %rze192 to i32
  %ub193 = bitcast i32 %uf194 to float
  store float %ub193, ptr %ip_29, align 4
  %ip_30 = getelementptr inbounds %State, ptr %state, i32 0, i32 30
%ff198 = bitcast i32 987065018 to float
  %il200 = load float, float* @dpy, align 4
%t201 = fmul fast float %ff198, %il200
  %rbi202 = bitcast float %t201 to i32
  %rze203 = zext i32 %rbi202 to i64
  %uf205 = trunc i64 %rze203 to i32
  %ub204 = bitcast i32 %uf205 to float
  store float %ub204, ptr %ip_30, align 4
  %ip_31 = getelementptr inbounds %State, ptr %state, i32 0, i32 31
%ff211 = bitcast i32 952602680 to float
  %t212 = fneg float %ff211
  %il214 = load float, float* @dpy, align 4
%t215 = fmul fast float %t212, %il214
  %rbi216 = bitcast float %t215 to i32
  %rze217 = zext i32 %rbi216 to i64
  %uf219 = trunc i64 %rze217 to i32
  %ub218 = bitcast i32 %uf219 to float
  store float %ub218, ptr %ip_31, align 4
  %ip_32 = getelementptr inbounds %State, ptr %state, i32 0, i32 32
  %ip_32b = bitcast i32 0 to float
  store float %ip_32b, ptr %ip_32, align 4
  %ip_33 = getelementptr inbounds %State, ptr %state, i32 0, i32 33
  store i64 0, ptr %ip_33, align 8
  ret void
}


define i32 @main() local_unnamed_addr #5 {
  entry:
  %state_0 = alloca %StateChunk0, align 8
  %state_1 = alloca %StateChunk1, align 8
  %state_2 = alloca %StateChunk2, align 8
  %state = alloca %State, align 8
  %ip_220 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
%sp224 = getelementptr inbounds [6 x i8], [6 x i8]* @str.3, i64 0, i64 0
%t223 = ptrtoint i8* %sp224 to i64
  %gsr225 = inttoptr i64 %t223 to ptr
  %gsp226 = bitcast ptr %gsr225 to ptr
  %gdp227 = load i64, ptr %gsp226, align 8
  %gnp228 = inttoptr i64 %gdp227 to ptr
  %gnv229 = call ptr @getenv(ptr %gnp228)
  %gnvl230 = icmp eq ptr %gnv229, null
  br i1 %gnvl230, label %genv_nul231, label %genv_ok232
  genv_nul231:
    br label %genv_af233
  genv_ok232:
  %gav234 = call i64 @atol(ptr %gnv229)
    br label %genv_af233
  genv_af233:
  %t221 = phi i64 [ 0, %genv_nul231 ], [ %gav234, %genv_ok232 ]
  store i64 %t221, ptr %ip_220, align 8
  %ip_235 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
%t237 = add i64 0, 0
  store i64 %t237, ptr %ip_235, align 8
  %ip_238 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %ip_2b = bitcast i32 0 to float
  store float %ip_2b, ptr %ip_238, align 4
  %ip_239 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %ip_3b = bitcast i32 0 to float
  store float %ip_3b, ptr %ip_239, align 4
  %ip_240 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %ip_4b = bitcast i32 0 to float
  store float %ip_4b, ptr %ip_240, align 4
  %ip_241 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %ip_5b = bitcast i32 0 to float
  store float %ip_5b, ptr %ip_241, align 4
  %ip_242 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %ip_6b = bitcast i32 0 to float
  store float %ip_6b, ptr %ip_242, align 4
  %ip_243 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
  %ip_7b = bitcast i32 0 to float
  store float %ip_7b, ptr %ip_243, align 4
  %ip_244 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
  %ip_8b = bitcast i32 1083895042 to float
  store float %ip_8b, ptr %ip_244, align 4
  %ip_245 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
%ff250 = bitcast i32 1066698078 to float
  %t251 = fneg float %ff250
  %rbi252 = bitcast float %t251 to i32
  %rze253 = zext i32 %rbi252 to i64
  %uf255 = trunc i64 %rze253 to i32
  %ub254 = bitcast i32 %uf255 to float
  store float %ub254, ptr %ip_245, align 4
  %ip_256 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
%ff261 = bitcast i32 1037318091 to float
  %t262 = fneg float %ff261
  %rbi263 = bitcast float %t262 to i32
  %rze264 = zext i32 %rbi263 to i64
  %uf266 = trunc i64 %rze264 to i32
  %ub265 = bitcast i32 %uf266 to float
  store float %ub265, ptr %ip_256, align 4
  %ip_267 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
%ff271 = bitcast i32 987338478 to float
  %il273 = load float, float* @dpy, align 4
%t274 = fmul fast float %ff271, %il273
  %rbi275 = bitcast float %t274 to i32
  %rze276 = zext i32 %rbi275 to i64
  %uf278 = trunc i64 %rze276 to i32
  %ub277 = bitcast i32 %uf278 to float
  store float %ub277, ptr %ip_267, align 4
  %ip_279 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
%ff283 = bitcast i32 1006389245 to float
  %il285 = load float, float* @dpy, align 4
%t286 = fmul fast float %ff283, %il285
  %rbi287 = bitcast float %t286 to i32
  %rze288 = zext i32 %rbi287 to i64
  %uf290 = trunc i64 %rze288 to i32
  %ub289 = bitcast i32 %uf290 to float
  store float %ub289, ptr %ip_279, align 4
  %ip_291 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
%ff297 = bitcast i32 949013706 to float
  %t298 = fneg float %ff297
  %il300 = load float, float* @dpy, align 4
%t301 = fmul fast float %t298, %il300
  %rbi302 = bitcast float %t301 to i32
  %rze303 = zext i32 %rbi302 to i64
  %uf305 = trunc i64 %rze303 to i32
  %ub304 = bitcast i32 %uf305 to float
  store float %ub304, ptr %ip_291, align 4
  %ip_306 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 14
  %ip_14b = bitcast i32 1090879086 to float
  store float %ip_14b, ptr %ip_306, align 4
  %ip_307 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 0
  %ip_15b = bitcast i32 1082392154 to float
  store float %ip_15b, ptr %ip_307, align 4
  %ip_308 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 1
%ff313 = bitcast i32 1053727391 to float
  %t314 = fneg float %ff313
  %rbi315 = bitcast float %t314 to i32
  %rze316 = zext i32 %rbi315 to i64
  %uf318 = trunc i64 %rze316 to i32
  %ub317 = bitcast i32 %uf318 to float
  store float %ub317, ptr %ip_308, align 4
  %ip_319 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 2
%ff325 = bitcast i32 993353136 to float
  %t326 = fneg float %ff325
  %il328 = load float, float* @dpy, align 4
%t329 = fmul fast float %t326, %il328
  %rbi330 = bitcast float %t329 to i32
  %rze331 = zext i32 %rbi330 to i64
  %uf333 = trunc i64 %rze331 to i32
  %ub332 = bitcast i32 %uf333 to float
  store float %ub332, ptr %ip_319, align 4
  %ip_334 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 3
%ff338 = bitcast i32 1000590001 to float
  %il340 = load float, float* @dpy, align 4
%t341 = fmul fast float %ff338, %il340
  %rbi342 = bitcast float %t341 to i32
  %rze343 = zext i32 %rbi342 to i64
  %uf345 = trunc i64 %rze343 to i32
  %ub344 = bitcast i32 %uf345 to float
  store float %ub344, ptr %ip_334, align 4
  %ip_346 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 4
%ff350 = bitcast i32 935414205 to float
  %il352 = load float, float* @dpy, align 4
%t353 = fmul fast float %ff350, %il352
  %rbi354 = bitcast float %t353 to i32
  %rze355 = zext i32 %rbi354 to i64
  %uf357 = trunc i64 %rze355 to i32
  %ub356 = bitcast i32 %uf357 to float
  store float %ub356, ptr %ip_346, align 4
  %ip_358 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 5
  %ip_20b = bitcast i32 1095651158 to float
  store float %ip_20b, ptr %ip_358, align 4
  %ip_359 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 6
%ff364 = bitcast i32 1097975623 to float
  %t365 = fneg float %ff364
  %rbi366 = bitcast float %t365 to i32
  %rze367 = zext i32 %rbi366 to i64
  %uf369 = trunc i64 %rze367 to i32
  %ub368 = bitcast i32 %uf369 to float
  store float %ub368, ptr %ip_359, align 4
  %ip_370 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 7
%ff375 = bitcast i32 1046784702 to float
  %t376 = fneg float %ff375
  %rbi377 = bitcast float %t376 to i32
  %rze378 = zext i32 %rbi377 to i64
  %uf380 = trunc i64 %rze378 to i32
  %ub379 = bitcast i32 %uf380 to float
  store float %ub379, ptr %ip_370, align 4
  %ip_381 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 8
%ff385 = bitcast i32 994200002 to float
  %il387 = load float, float* @dpy, align 4
%t388 = fmul fast float %ff385, %il387
  %rbi389 = bitcast float %t388 to i32
  %rze390 = zext i32 %rbi389 to i64
  %uf392 = trunc i64 %rze390 to i32
  %ub391 = bitcast i32 %uf392 to float
  store float %ub391, ptr %ip_381, align 4
  %ip_393 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 9
%ff397 = bitcast i32 991682594 to float
  %il399 = load float, float* @dpy, align 4
%t400 = fmul fast float %ff397, %il399
  %rbi401 = bitcast float %t400 to i32
  %rze402 = zext i32 %rbi401 to i64
  %uf404 = trunc i64 %rze402 to i32
  %ub403 = bitcast i32 %uf404 to float
  store float %ub403, ptr %ip_393, align 4
  %ip_405 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 10
%ff411 = bitcast i32 939052064 to float
  %t412 = fneg float %ff411
  %il414 = load float, float* @dpy, align 4
%t415 = fmul fast float %t412, %il414
  %rbi416 = bitcast float %t415 to i32
  %rze417 = zext i32 %rbi416 to i64
  %uf419 = trunc i64 %rze417 to i32
  %ub418 = bitcast i32 %uf419 to float
  store float %ub418, ptr %ip_405, align 4
  %ip_420 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 11
  %ip_26b = bitcast i32 1098257213 to float
  store float %ip_26b, ptr %ip_420, align 4
  %ip_421 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 12
%ff426 = bitcast i32 1104108226 to float
  %t427 = fneg float %ff426
  %rbi428 = bitcast float %t427 to i32
  %rze429 = zext i32 %rbi428 to i64
  %uf431 = trunc i64 %rze429 to i32
  %ub430 = bitcast i32 %uf431 to float
  store float %ub430, ptr %ip_421, align 4
  %ip_432 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 13
  %ip_28b = bitcast i32 1043828637 to float
  store float %ip_28b, ptr %ip_432, align 4
  %ip_433 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 14
%ff437 = bitcast i32 992980559 to float
  %il439 = load float, float* @dpy, align 4
%t440 = fmul fast float %ff437, %il439
  %rbi441 = bitcast float %t440 to i32
  %rze442 = zext i32 %rbi441 to i64
  %uf444 = trunc i64 %rze442 to i32
  %ub443 = bitcast i32 %uf444 to float
  store float %ub443, ptr %ip_433, align 4
  %ip_445 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 0
%ff449 = bitcast i32 987065018 to float
  %il451 = load float, float* @dpy, align 4
%t452 = fmul fast float %ff449, %il451
  %rbi453 = bitcast float %t452 to i32
  %rze454 = zext i32 %rbi453 to i64
  %uf456 = trunc i64 %rze454 to i32
  %ub455 = bitcast i32 %uf456 to float
  store float %ub455, ptr %ip_445, align 4
  %ip_457 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 1
%ff463 = bitcast i32 952602680 to float
  %t464 = fneg float %ff463
  %il466 = load float, float* @dpy, align 4
%t467 = fmul fast float %t464, %il466
  %rbi468 = bitcast float %t467 to i32
  %rze469 = zext i32 %rbi468 to i64
  %uf471 = trunc i64 %rze469 to i32
  %ub470 = bitcast i32 %uf471 to float
  store float %ub470, ptr %ip_457, align 4
  %ip_472 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 2
  %ip_32b = bitcast i32 0 to float
  store float %ip_32b, ptr %ip_472, align 4
  %ip_473 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 3
  store i64 0, ptr %ip_473, align 8
  %arptr3 = alloca i8*, align 8
  %arend3 = alloca i8*, align 8
  %arbase3 = alloca i8*, align 8
  %arinit3 = call ptr @malloc(i64 65536)
  store i8* %arinit3, ptr %arptr3, align 8
  store i8* %arinit3, ptr %arbase3, align 8
  %arieu3 = getelementptr i8, ptr %arinit3, i64 65536
  store i8* %arieu3, ptr %arend3, align 8
  %gt_474 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %cnt_bound_220 = load i64, ptr %gt_474, align 8
  br label %pre_phi
pre_phi:
  %lv_475 = alloca float, align 4
  %init_cnt_476 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %init_bound_477 = load i64, ptr %init_cnt_476, align 8, !tbaa !1
  %init_cnt_478 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %init_bx0_479 = load float, ptr %init_cnt_478, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_480 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
  %init_bx1_481 = load float, ptr %init_cnt_480, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_482 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 14
  %init_bx2_483 = load float, ptr %init_cnt_482, align 4, !tbaa !3
  %init_cnt_484 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 5
  %init_bx3_485 = load float, ptr %init_cnt_484, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_486 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 11
  %init_bx4_487 = load float, ptr %init_cnt_486, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_488 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %init_by0_489 = load float, ptr %init_cnt_488, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_490 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
  %init_by1_491 = load float, ptr %init_cnt_490, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_492 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 0
  %init_by2_493 = load float, ptr %init_cnt_492, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_494 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 6
  %init_by3_495 = load float, ptr %init_cnt_494, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_496 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 12
  %init_by4_497 = load float, ptr %init_cnt_496, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_498 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %init_bz0_499 = load float, ptr %init_cnt_498, align 4, !tbaa !3
  %init_cnt_500 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
  %init_bz1_501 = load float, ptr %init_cnt_500, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_502 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 1
  %init_bz2_503 = load float, ptr %init_cnt_502, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_504 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 7
  %init_bz3_505 = load float, ptr %init_cnt_504, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_506 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 13
  %init_bz4_507 = load float, ptr %init_cnt_506, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_508 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 3
  %init_cycle_count_509 = load i64, ptr %init_cnt_508, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_510 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 2
  %init_last_energy_511 = load float, ptr %init_cnt_510, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_512 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %init_vx0_513 = load float, ptr %init_cnt_512, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_514 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
  %init_vx1_515 = load float, ptr %init_cnt_514, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_516 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 2
  %init_vx2_517 = load float, ptr %init_cnt_516, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_518 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 8
  %init_vx3_519 = load float, ptr %init_cnt_518, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_520 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 14
  %init_vx4_521 = load float, ptr %init_cnt_520, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_522 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %init_vy0_523 = load float, ptr %init_cnt_522, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_524 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
  %init_vy1_525 = load float, ptr %init_cnt_524, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_526 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 3
  %init_vy2_527 = load float, ptr %init_cnt_526, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_528 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 9
  %init_vy3_529 = load float, ptr %init_cnt_528, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_530 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 0
  %init_vy4_531 = load float, ptr %init_cnt_530, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_532 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
  %init_vz0_533 = load float, ptr %init_cnt_532, align 4, !tbaa !3
  %init_cnt_534 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
  %init_vz1_535 = load float, ptr %init_cnt_534, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_536 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 4
  %init_vz2_537 = load float, ptr %init_cnt_536, align 4, !tbaa !3
  %init_cnt_538 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 10
  %init_vz3_539 = load float, ptr %init_cnt_538, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_540 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 1
  %init_vz4_541 = load float, ptr %init_cnt_540, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_542 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
  %init_count_543 = load i64, ptr %init_cnt_542, align 8
  %iv545_phi_bx_v4545 = insertelement <4 x float> undef, float %init_bx0_479, i32 0
  %iv546_phi_bx_v4546 = insertelement <4 x float> %iv545_phi_bx_v4545, float %init_bx1_481, i32 1
  %iv547_phi_bx_v4547 = insertelement <4 x float> %iv546_phi_bx_v4546, float %init_bx2_483, i32 2
  %iv548_phi_bx_v4548 = insertelement <4 x float> %iv547_phi_bx_v4547, float %init_bx3_485, i32 3
  %iv549_phi_vz_v4549 = insertelement <4 x float> undef, float %init_vz0_533, i32 0
  %iv550_phi_vz_v4550 = insertelement <4 x float> %iv549_phi_vz_v4549, float %init_vz1_535, i32 1
  %iv551_phi_vz_v4551 = insertelement <4 x float> %iv550_phi_vz_v4550, float %init_vz2_537, i32 2
  %iv552_phi_vz_v4552 = insertelement <4 x float> %iv551_phi_vz_v4551, float %init_vz3_539, i32 3
  %iv553_phi_bz_v4553 = insertelement <4 x float> undef, float %init_bz0_499, i32 0
  %iv554_phi_bz_v4554 = insertelement <4 x float> %iv553_phi_bz_v4553, float %init_bz1_501, i32 1
  %iv555_phi_bz_v4555 = insertelement <4 x float> %iv554_phi_bz_v4554, float %init_bz2_503, i32 2
  %iv556_phi_bz_v4556 = insertelement <4 x float> %iv555_phi_bz_v4555, float %init_bz3_505, i32 3
  %iv557_phi_vy_v4557 = insertelement <4 x float> undef, float %init_vy0_523, i32 0
  %iv558_phi_vy_v4558 = insertelement <4 x float> %iv557_phi_vy_v4557, float %init_vy1_525, i32 1
  %iv559_phi_vy_v4559 = insertelement <4 x float> %iv558_phi_vy_v4558, float %init_vy2_527, i32 2
  %iv560_phi_vy_v4560 = insertelement <4 x float> %iv559_phi_vy_v4559, float %init_vy3_529, i32 3
  %iv561_phi_vx_v4561 = insertelement <4 x float> undef, float %init_vx0_513, i32 0
  %iv562_phi_vx_v4562 = insertelement <4 x float> %iv561_phi_vx_v4561, float %init_vx1_515, i32 1
  %iv563_phi_vx_v4563 = insertelement <4 x float> %iv562_phi_vx_v4562, float %init_vx2_517, i32 2
  %iv564_phi_vx_v4564 = insertelement <4 x float> %iv563_phi_vx_v4563, float %init_vx3_519, i32 3
  %iv565_phi_by_v4565 = insertelement <4 x float> undef, float %init_by0_489, i32 0
  %iv566_phi_by_v4566 = insertelement <4 x float> %iv565_phi_by_v4565, float %init_by1_491, i32 1
  %iv567_phi_by_v4567 = insertelement <4 x float> %iv566_phi_by_v4566, float %init_by2_493, i32 2
  %iv568_phi_by_v4568 = insertelement <4 x float> %iv567_phi_by_v4567, float %init_by3_495, i32 3
  br label %loop_hdr
loop_hdr:
  %pi_cnt_544 = phi i64 [ %init_count_543, %pre_phi ], [ %pn_cnt_544, %latch ]
  %phi_bx4 = phi float [ %init_bx4_487, %pre_phi ], [ %be_bx4, %latch ]
  %phi_vz_v4 = phi <4 x float> [ %iv552_phi_vz_v4552, %pre_phi ], [ %be_vz_v4, %latch ]
  %phi_bz_v4 = phi <4 x float> [ %iv556_phi_bz_v4556, %pre_phi ], [ %be_bz_v4, %latch ]
  %phi_last_energy = phi float [ %init_last_energy_511, %pre_phi ], [ %be_last_energy, %latch ]
  %phi_vx_v4 = phi <4 x float> [ %iv564_phi_vx_v4564, %pre_phi ], [ %be_vx_v4, %latch ]
  %phi_by_v4 = phi <4 x float> [ %iv568_phi_by_v4568, %pre_phi ], [ %be_by_v4, %latch ]
  %phi_vy4 = phi float [ %init_vy4_531, %pre_phi ], [ %be_vy4, %latch ]
  %phi_vx4 = phi float [ %init_vx4_521, %pre_phi ], [ %be_vx4, %latch ]
  %phi_vy_v4 = phi <4 x float> [ %iv560_phi_vy_v4560, %pre_phi ], [ %be_vy_v4, %latch ]
  %phi_bx_v4 = phi <4 x float> [ %iv548_phi_bx_v4548, %pre_phi ], [ %be_bx_v4, %latch ]
  %phi_by4 = phi float [ %init_by4_497, %pre_phi ], [ %be_by4, %latch ]
  %phi_cycle_count = phi i64 [ %init_cycle_count_509, %pre_phi ], [ %be_cycle_count, %latch ]
  %phi_vz4 = phi float [ %init_vz4_541, %pre_phi ], [ %be_vz4, %latch ]
  %phi_bound = phi i64 [ %init_bound_477, %pre_phi ], [ %be_bound, %latch ]
  %phi_bz4 = phi float [ %init_bz4_507, %pre_phi ], [ %be_bz4, %latch ]
  %phi_count = phi i64 [ %init_count_543, %pre_phi ], [ %be_count, %latch ]
  %cmp_hdr_569 = icmp slt i64 %pi_cnt_544, %cnt_bound_220
  br i1 %cmp_hdr_569, label %body, label %commit
body:
  %phi_bx_e0 = extractelement <4 x float> %phi_bx_v4, i32 0
  %phi_bx_e1 = extractelement <4 x float> %phi_bx_v4, i32 1
  %phi_bx_e2 = extractelement <4 x float> %phi_bx_v4, i32 2
  %phi_bx_e3 = extractelement <4 x float> %phi_bx_v4, i32 3
  %phi_by_e0 = extractelement <4 x float> %phi_by_v4, i32 0
  %phi_by_e1 = extractelement <4 x float> %phi_by_v4, i32 1
  %phi_by_e2 = extractelement <4 x float> %phi_by_v4, i32 2
  %phi_by_e3 = extractelement <4 x float> %phi_by_v4, i32 3
  %phi_bz_e0 = extractelement <4 x float> %phi_bz_v4, i32 0
  %phi_bz_e1 = extractelement <4 x float> %phi_bz_v4, i32 1
  %phi_bz_e2 = extractelement <4 x float> %phi_bz_v4, i32 2
  %phi_bz_e3 = extractelement <4 x float> %phi_bz_v4, i32 3
  %phi_vx_e0 = extractelement <4 x float> %phi_vx_v4, i32 0
  %phi_vx_e1 = extractelement <4 x float> %phi_vx_v4, i32 1
  %phi_vx_e2 = extractelement <4 x float> %phi_vx_v4, i32 2
  %phi_vx_e3 = extractelement <4 x float> %phi_vx_v4, i32 3
  %phi_vy_e0 = extractelement <4 x float> %phi_vy_v4, i32 0
  %phi_vy_e1 = extractelement <4 x float> %phi_vy_v4, i32 1
  %phi_vy_e2 = extractelement <4 x float> %phi_vy_v4, i32 2
  %phi_vy_e3 = extractelement <4 x float> %phi_vy_v4, i32 3
  %phi_vz_e0 = extractelement <4 x float> %phi_vz_v4, i32 0
  %phi_vz_e1 = extractelement <4 x float> %phi_vz_v4, i32 1
  %phi_vz_e2 = extractelement <4 x float> %phi_vz_v4, i32 2
  %phi_vz_e3 = extractelement <4 x float> %phi_vz_v4, i32 3
%t573 = fsub fast float %phi_bx_e0, %phi_bx_e1
  ; let dx01 = %t573
%t577 = fsub fast float %phi_by_e0, %phi_by_e1
  ; let dy01 = %t577
%t581 = fsub fast float %phi_bz_e0, %phi_bz_e1
  ; let dz01 = %t581
%t587 = fmul fast float %t573, %t573
%t591 = fmul fast float %t577, %t577
%t592 = fadd fast float %t587, %t591
%t596 = fmul fast float %t581, %t581
%t597 = fadd fast float %t592, %t596
  ; let dsq01 = %t597
%ff602 = bitcast i32 1056964608 to float
%t603 = fmul fast float %t597, %ff602
  ; let dist01a = %t603
%ff607 = bitcast i32 1056964608 to float
%t613 = fdiv fast float %t597, %t603
%t614 = fadd fast float %t603, %t613
%t615 = fmul fast float %ff607, %t614
  ; let dist01b = %t615
%ff619 = bitcast i32 1056964608 to float
%t625 = fdiv fast float %t597, %t615
%t626 = fadd fast float %t615, %t625
%t627 = fmul fast float %ff619, %t626
  ; let dist01c = %t627
%ff631 = bitcast i32 1056964608 to float
%t637 = fdiv fast float %t597, %t627
%t638 = fadd fast float %t627, %t637
%t639 = fmul fast float %ff631, %t638
  ; let dist01d = %t639
%ff643 = bitcast i32 1056964608 to float
%t649 = fdiv fast float %t597, %t639
%t650 = fadd fast float %t639, %t649
%t651 = fmul fast float %ff643, %t650
  ; let dist01e = %t651
%ff655 = bitcast i32 1056964608 to float
%t661 = fdiv fast float %t597, %t651
%t662 = fadd fast float %t651, %t661
%t663 = fmul fast float %ff655, %t662
  ; let dist01 = %t663
  %il666 = load float, float* @dt, align 4
%t670 = fmul fast float %t597, %t663
%t671 = fdiv fast float %il666, %t670
  ; let mag01 = %t671
%t675 = fsub fast float %phi_bx_e0, %phi_bx_e2
  ; let dx02 = %t675
%t679 = fsub fast float %phi_by_e0, %phi_by_e2
  ; let dy02 = %t679
%t683 = fsub fast float %phi_bz_e0, %phi_bz_e2
  ; let dz02 = %t683
%t689 = fmul fast float %t675, %t675
%t693 = fmul fast float %t679, %t679
%t694 = fadd fast float %t689, %t693
%t698 = fmul fast float %t683, %t683
%t699 = fadd fast float %t694, %t698
  ; let dsq02 = %t699
%ff704 = bitcast i32 1056964608 to float
%t705 = fmul fast float %t699, %ff704
  ; let dist02a = %t705
%ff709 = bitcast i32 1056964608 to float
%t715 = fdiv fast float %t699, %t705
%t716 = fadd fast float %t705, %t715
%t717 = fmul fast float %ff709, %t716
  ; let dist02b = %t717
%ff721 = bitcast i32 1056964608 to float
%t727 = fdiv fast float %t699, %t717
%t728 = fadd fast float %t717, %t727
%t729 = fmul fast float %ff721, %t728
  ; let dist02c = %t729
%ff733 = bitcast i32 1056964608 to float
%t739 = fdiv fast float %t699, %t729
%t740 = fadd fast float %t729, %t739
%t741 = fmul fast float %ff733, %t740
  ; let dist02d = %t741
%ff745 = bitcast i32 1056964608 to float
%t751 = fdiv fast float %t699, %t741
%t752 = fadd fast float %t741, %t751
%t753 = fmul fast float %ff745, %t752
  ; let dist02e = %t753
%ff757 = bitcast i32 1056964608 to float
%t763 = fdiv fast float %t699, %t753
%t764 = fadd fast float %t753, %t763
%t765 = fmul fast float %ff757, %t764
  ; let dist02 = %t765
  %il768 = load float, float* @dt, align 4
%t772 = fmul fast float %t699, %t765
%t773 = fdiv fast float %il768, %t772
  ; let mag02 = %t773
%t777 = fsub fast float %phi_bx_e0, %phi_bx_e3
  ; let dx03 = %t777
%t781 = fsub fast float %phi_by_e0, %phi_by_e3
  ; let dy03 = %t781
%t785 = fsub fast float %phi_bz_e0, %phi_bz_e3
  ; let dz03 = %t785
%t791 = fmul fast float %t777, %t777
%t795 = fmul fast float %t781, %t781
%t796 = fadd fast float %t791, %t795
%t800 = fmul fast float %t785, %t785
%t801 = fadd fast float %t796, %t800
  ; let dsq03 = %t801
%ff806 = bitcast i32 1056964608 to float
%t807 = fmul fast float %t801, %ff806
  ; let dist03a = %t807
%ff811 = bitcast i32 1056964608 to float
%t817 = fdiv fast float %t801, %t807
%t818 = fadd fast float %t807, %t817
%t819 = fmul fast float %ff811, %t818
  ; let dist03b = %t819
%ff823 = bitcast i32 1056964608 to float
%t829 = fdiv fast float %t801, %t819
%t830 = fadd fast float %t819, %t829
%t831 = fmul fast float %ff823, %t830
  ; let dist03c = %t831
%ff835 = bitcast i32 1056964608 to float
%t841 = fdiv fast float %t801, %t831
%t842 = fadd fast float %t831, %t841
%t843 = fmul fast float %ff835, %t842
  ; let dist03d = %t843
%ff847 = bitcast i32 1056964608 to float
%t853 = fdiv fast float %t801, %t843
%t854 = fadd fast float %t843, %t853
%t855 = fmul fast float %ff847, %t854
  ; let dist03e = %t855
%ff859 = bitcast i32 1056964608 to float
%t865 = fdiv fast float %t801, %t855
%t866 = fadd fast float %t855, %t865
%t867 = fmul fast float %ff859, %t866
  ; let dist03 = %t867
  %il870 = load float, float* @dt, align 4
%t874 = fmul fast float %t801, %t867
%t875 = fdiv fast float %il870, %t874
  ; let mag03 = %t875
%t879 = fsub fast float %phi_bx_e0, %phi_bx4
  ; let dx04 = %t879
%t883 = fsub fast float %phi_by_e0, %phi_by4
  ; let dy04 = %t883
%t887 = fsub fast float %phi_bz_e0, %phi_bz4
  ; let dz04 = %t887
%t893 = fmul fast float %t879, %t879
%t897 = fmul fast float %t883, %t883
%t898 = fadd fast float %t893, %t897
%t902 = fmul fast float %t887, %t887
%t903 = fadd fast float %t898, %t902
  ; let dsq04 = %t903
%ff908 = bitcast i32 1056964608 to float
%t909 = fmul fast float %t903, %ff908
  ; let dist04a = %t909
%ff913 = bitcast i32 1056964608 to float
%t919 = fdiv fast float %t903, %t909
%t920 = fadd fast float %t909, %t919
%t921 = fmul fast float %ff913, %t920
  ; let dist04b = %t921
%ff925 = bitcast i32 1056964608 to float
%t931 = fdiv fast float %t903, %t921
%t932 = fadd fast float %t921, %t931
%t933 = fmul fast float %ff925, %t932
  ; let dist04c = %t933
%ff937 = bitcast i32 1056964608 to float
%t943 = fdiv fast float %t903, %t933
%t944 = fadd fast float %t933, %t943
%t945 = fmul fast float %ff937, %t944
  ; let dist04d = %t945
%ff949 = bitcast i32 1056964608 to float
%t955 = fdiv fast float %t903, %t945
%t956 = fadd fast float %t945, %t955
%t957 = fmul fast float %ff949, %t956
  ; let dist04e = %t957
%ff961 = bitcast i32 1056964608 to float
%t967 = fdiv fast float %t903, %t957
%t968 = fadd fast float %t957, %t967
%t969 = fmul fast float %ff961, %t968
  ; let dist04 = %t969
  %il972 = load float, float* @dt, align 4
%t976 = fmul fast float %t903, %t969
%t977 = fdiv fast float %il972, %t976
  ; let mag04 = %t977
%t981 = fsub fast float %phi_bx_e1, %phi_bx_e2
  ; let dx12 = %t981
%t985 = fsub fast float %phi_by_e1, %phi_by_e2
  ; let dy12 = %t985
%t989 = fsub fast float %phi_bz_e1, %phi_bz_e2
  ; let dz12 = %t989
%t995 = fmul fast float %t981, %t981
%t999 = fmul fast float %t985, %t985
%t1000 = fadd fast float %t995, %t999
%t1004 = fmul fast float %t989, %t989
%t1005 = fadd fast float %t1000, %t1004
  ; let dsq12 = %t1005
%ff1010 = bitcast i32 1056964608 to float
%t1011 = fmul fast float %t1005, %ff1010
  ; let dist12a = %t1011
%ff1015 = bitcast i32 1056964608 to float
%t1021 = fdiv fast float %t1005, %t1011
%t1022 = fadd fast float %t1011, %t1021
%t1023 = fmul fast float %ff1015, %t1022
  ; let dist12b = %t1023
%ff1027 = bitcast i32 1056964608 to float
%t1033 = fdiv fast float %t1005, %t1023
%t1034 = fadd fast float %t1023, %t1033
%t1035 = fmul fast float %ff1027, %t1034
  ; let dist12c = %t1035
%ff1039 = bitcast i32 1056964608 to float
%t1045 = fdiv fast float %t1005, %t1035
%t1046 = fadd fast float %t1035, %t1045
%t1047 = fmul fast float %ff1039, %t1046
  ; let dist12d = %t1047
%ff1051 = bitcast i32 1056964608 to float
%t1057 = fdiv fast float %t1005, %t1047
%t1058 = fadd fast float %t1047, %t1057
%t1059 = fmul fast float %ff1051, %t1058
  ; let dist12e = %t1059
%ff1063 = bitcast i32 1056964608 to float
%t1069 = fdiv fast float %t1005, %t1059
%t1070 = fadd fast float %t1059, %t1069
%t1071 = fmul fast float %ff1063, %t1070
  ; let dist12 = %t1071
  %il1074 = load float, float* @dt, align 4
%t1078 = fmul fast float %t1005, %t1071
%t1079 = fdiv fast float %il1074, %t1078
  ; let mag12 = %t1079
%t1083 = fsub fast float %phi_bx_e1, %phi_bx_e3
  ; let dx13 = %t1083
%t1087 = fsub fast float %phi_by_e1, %phi_by_e3
  ; let dy13 = %t1087
%t1091 = fsub fast float %phi_bz_e1, %phi_bz_e3
  ; let dz13 = %t1091
%t1097 = fmul fast float %t1083, %t1083
%t1101 = fmul fast float %t1087, %t1087
%t1102 = fadd fast float %t1097, %t1101
%t1106 = fmul fast float %t1091, %t1091
%t1107 = fadd fast float %t1102, %t1106
  ; let dsq13 = %t1107
%ff1112 = bitcast i32 1056964608 to float
%t1113 = fmul fast float %t1107, %ff1112
  ; let dist13a = %t1113
%ff1117 = bitcast i32 1056964608 to float
%t1123 = fdiv fast float %t1107, %t1113
%t1124 = fadd fast float %t1113, %t1123
%t1125 = fmul fast float %ff1117, %t1124
  ; let dist13b = %t1125
%ff1129 = bitcast i32 1056964608 to float
%t1135 = fdiv fast float %t1107, %t1125
%t1136 = fadd fast float %t1125, %t1135
%t1137 = fmul fast float %ff1129, %t1136
  ; let dist13c = %t1137
%ff1141 = bitcast i32 1056964608 to float
%t1147 = fdiv fast float %t1107, %t1137
%t1148 = fadd fast float %t1137, %t1147
%t1149 = fmul fast float %ff1141, %t1148
  ; let dist13d = %t1149
%ff1153 = bitcast i32 1056964608 to float
%t1159 = fdiv fast float %t1107, %t1149
%t1160 = fadd fast float %t1149, %t1159
%t1161 = fmul fast float %ff1153, %t1160
  ; let dist13e = %t1161
%ff1165 = bitcast i32 1056964608 to float
%t1171 = fdiv fast float %t1107, %t1161
%t1172 = fadd fast float %t1161, %t1171
%t1173 = fmul fast float %ff1165, %t1172
  ; let dist13 = %t1173
  %il1176 = load float, float* @dt, align 4
%t1180 = fmul fast float %t1107, %t1173
%t1181 = fdiv fast float %il1176, %t1180
  ; let mag13 = %t1181
%t1185 = fsub fast float %phi_bx_e1, %phi_bx4
  ; let dx14 = %t1185
%t1189 = fsub fast float %phi_by_e1, %phi_by4
  ; let dy14 = %t1189
%t1193 = fsub fast float %phi_bz_e1, %phi_bz4
  ; let dz14 = %t1193
%t1199 = fmul fast float %t1185, %t1185
%t1203 = fmul fast float %t1189, %t1189
%t1204 = fadd fast float %t1199, %t1203
%t1208 = fmul fast float %t1193, %t1193
%t1209 = fadd fast float %t1204, %t1208
  ; let dsq14 = %t1209
%ff1214 = bitcast i32 1056964608 to float
%t1215 = fmul fast float %t1209, %ff1214
  ; let dist14a = %t1215
%ff1219 = bitcast i32 1056964608 to float
%t1225 = fdiv fast float %t1209, %t1215
%t1226 = fadd fast float %t1215, %t1225
%t1227 = fmul fast float %ff1219, %t1226
  ; let dist14b = %t1227
%ff1231 = bitcast i32 1056964608 to float
%t1237 = fdiv fast float %t1209, %t1227
%t1238 = fadd fast float %t1227, %t1237
%t1239 = fmul fast float %ff1231, %t1238
  ; let dist14c = %t1239
%ff1243 = bitcast i32 1056964608 to float
%t1249 = fdiv fast float %t1209, %t1239
%t1250 = fadd fast float %t1239, %t1249
%t1251 = fmul fast float %ff1243, %t1250
  ; let dist14d = %t1251
%ff1255 = bitcast i32 1056964608 to float
%t1261 = fdiv fast float %t1209, %t1251
%t1262 = fadd fast float %t1251, %t1261
%t1263 = fmul fast float %ff1255, %t1262
  ; let dist14e = %t1263
%ff1267 = bitcast i32 1056964608 to float
%t1273 = fdiv fast float %t1209, %t1263
%t1274 = fadd fast float %t1263, %t1273
%t1275 = fmul fast float %ff1267, %t1274
  ; let dist14 = %t1275
  %il1278 = load float, float* @dt, align 4
%t1282 = fmul fast float %t1209, %t1275
%t1283 = fdiv fast float %il1278, %t1282
  ; let mag14 = %t1283
%t1287 = fsub fast float %phi_bx_e2, %phi_bx_e3
  ; let dx23 = %t1287
%t1291 = fsub fast float %phi_by_e2, %phi_by_e3
  ; let dy23 = %t1291
%t1295 = fsub fast float %phi_bz_e2, %phi_bz_e3
  ; let dz23 = %t1295
%t1301 = fmul fast float %t1287, %t1287
%t1305 = fmul fast float %t1291, %t1291
%t1306 = fadd fast float %t1301, %t1305
%t1310 = fmul fast float %t1295, %t1295
%t1311 = fadd fast float %t1306, %t1310
  ; let dsq23 = %t1311
%ff1316 = bitcast i32 1056964608 to float
%t1317 = fmul fast float %t1311, %ff1316
  ; let dist23a = %t1317
%ff1321 = bitcast i32 1056964608 to float
%t1327 = fdiv fast float %t1311, %t1317
%t1328 = fadd fast float %t1317, %t1327
%t1329 = fmul fast float %ff1321, %t1328
  ; let dist23b = %t1329
%ff1333 = bitcast i32 1056964608 to float
%t1339 = fdiv fast float %t1311, %t1329
%t1340 = fadd fast float %t1329, %t1339
%t1341 = fmul fast float %ff1333, %t1340
  ; let dist23c = %t1341
%ff1345 = bitcast i32 1056964608 to float
%t1351 = fdiv fast float %t1311, %t1341
%t1352 = fadd fast float %t1341, %t1351
%t1353 = fmul fast float %ff1345, %t1352
  ; let dist23d = %t1353
%ff1357 = bitcast i32 1056964608 to float
%t1363 = fdiv fast float %t1311, %t1353
%t1364 = fadd fast float %t1353, %t1363
%t1365 = fmul fast float %ff1357, %t1364
  ; let dist23e = %t1365
%ff1369 = bitcast i32 1056964608 to float
%t1375 = fdiv fast float %t1311, %t1365
%t1376 = fadd fast float %t1365, %t1375
%t1377 = fmul fast float %ff1369, %t1376
  ; let dist23 = %t1377
  %il1380 = load float, float* @dt, align 4
%t1384 = fmul fast float %t1311, %t1377
%t1385 = fdiv fast float %il1380, %t1384
  ; let mag23 = %t1385
%t1389 = fsub fast float %phi_bx_e2, %phi_bx4
  ; let dx24 = %t1389
%t1393 = fsub fast float %phi_by_e2, %phi_by4
  ; let dy24 = %t1393
%t1397 = fsub fast float %phi_bz_e2, %phi_bz4
  ; let dz24 = %t1397
%t1403 = fmul fast float %t1389, %t1389
%t1407 = fmul fast float %t1393, %t1393
%t1408 = fadd fast float %t1403, %t1407
%t1412 = fmul fast float %t1397, %t1397
%t1413 = fadd fast float %t1408, %t1412
  ; let dsq24 = %t1413
%ff1418 = bitcast i32 1056964608 to float
%t1419 = fmul fast float %t1413, %ff1418
  ; let dist24a = %t1419
%ff1423 = bitcast i32 1056964608 to float
%t1429 = fdiv fast float %t1413, %t1419
%t1430 = fadd fast float %t1419, %t1429
%t1431 = fmul fast float %ff1423, %t1430
  ; let dist24b = %t1431
%ff1435 = bitcast i32 1056964608 to float
%t1441 = fdiv fast float %t1413, %t1431
%t1442 = fadd fast float %t1431, %t1441
%t1443 = fmul fast float %ff1435, %t1442
  ; let dist24c = %t1443
%ff1447 = bitcast i32 1056964608 to float
%t1453 = fdiv fast float %t1413, %t1443
%t1454 = fadd fast float %t1443, %t1453
%t1455 = fmul fast float %ff1447, %t1454
  ; let dist24d = %t1455
%ff1459 = bitcast i32 1056964608 to float
%t1465 = fdiv fast float %t1413, %t1455
%t1466 = fadd fast float %t1455, %t1465
%t1467 = fmul fast float %ff1459, %t1466
  ; let dist24e = %t1467
%ff1471 = bitcast i32 1056964608 to float
%t1477 = fdiv fast float %t1413, %t1467
%t1478 = fadd fast float %t1467, %t1477
%t1479 = fmul fast float %ff1471, %t1478
  ; let dist24 = %t1479
  %il1482 = load float, float* @dt, align 4
%t1486 = fmul fast float %t1413, %t1479
%t1487 = fdiv fast float %il1482, %t1486
  ; let mag24 = %t1487
%t1491 = fsub fast float %phi_bx_e3, %phi_bx4
  ; let dx34 = %t1491
%t1495 = fsub fast float %phi_by_e3, %phi_by4
  ; let dy34 = %t1495
%t1499 = fsub fast float %phi_bz_e3, %phi_bz4
  ; let dz34 = %t1499
%t1505 = fmul fast float %t1491, %t1491
%t1509 = fmul fast float %t1495, %t1495
%t1510 = fadd fast float %t1505, %t1509
%t1514 = fmul fast float %t1499, %t1499
%t1515 = fadd fast float %t1510, %t1514
  ; let dsq34 = %t1515
%ff1520 = bitcast i32 1056964608 to float
%t1521 = fmul fast float %t1515, %ff1520
  ; let dist34a = %t1521
%ff1525 = bitcast i32 1056964608 to float
%t1531 = fdiv fast float %t1515, %t1521
%t1532 = fadd fast float %t1521, %t1531
%t1533 = fmul fast float %ff1525, %t1532
  ; let dist34b = %t1533
%ff1537 = bitcast i32 1056964608 to float
%t1543 = fdiv fast float %t1515, %t1533
%t1544 = fadd fast float %t1533, %t1543
%t1545 = fmul fast float %ff1537, %t1544
  ; let dist34c = %t1545
%ff1549 = bitcast i32 1056964608 to float
%t1555 = fdiv fast float %t1515, %t1545
%t1556 = fadd fast float %t1545, %t1555
%t1557 = fmul fast float %ff1549, %t1556
  ; let dist34d = %t1557
%ff1561 = bitcast i32 1056964608 to float
%t1567 = fdiv fast float %t1515, %t1557
%t1568 = fadd fast float %t1557, %t1567
%t1569 = fmul fast float %ff1561, %t1568
  ; let dist34e = %t1569
%ff1573 = bitcast i32 1056964608 to float
%t1579 = fdiv fast float %t1515, %t1569
%t1580 = fadd fast float %t1569, %t1579
%t1581 = fmul fast float %ff1573, %t1580
  ; let dist34 = %t1581
  %il1584 = load float, float* @dt, align 4
%t1588 = fmul fast float %t1515, %t1581
%t1589 = fdiv fast float %il1584, %t1588
  ; let mag34 = %t1589
  %il1599 = load float, float* @m1, align 4
%t1600 = fmul fast float %t573, %il1599
%t1602 = fmul fast float %t1600, %t671
%t1603 = fsub fast float %phi_vx_e0, %t1602
  %il1608 = load float, float* @m2, align 4
%t1609 = fmul fast float %t675, %il1608
%t1611 = fmul fast float %t1609, %t773
%t1612 = fsub fast float %t1603, %t1611
  %il1617 = load float, float* @m3, align 4
%t1618 = fmul fast float %t777, %il1617
%t1620 = fmul fast float %t1618, %t875
%t1621 = fsub fast float %t1612, %t1620
  %il1626 = load float, float* @m4, align 4
%t1627 = fmul fast float %t879, %il1626
%t1629 = fmul fast float %t1627, %t977
%t1630 = fsub fast float %t1621, %t1629
  ; let nvx0 = %t1630
  %il1640 = load float, float* @m1, align 4
%t1641 = fmul fast float %t577, %il1640
%t1643 = fmul fast float %t1641, %t671
%t1644 = fsub fast float %phi_vy_e0, %t1643
  %il1649 = load float, float* @m2, align 4
%t1650 = fmul fast float %t679, %il1649
%t1652 = fmul fast float %t1650, %t773
%t1653 = fsub fast float %t1644, %t1652
  %il1658 = load float, float* @m3, align 4
%t1659 = fmul fast float %t781, %il1658
%t1661 = fmul fast float %t1659, %t875
%t1662 = fsub fast float %t1653, %t1661
  %il1667 = load float, float* @m4, align 4
%t1668 = fmul fast float %t883, %il1667
%t1670 = fmul fast float %t1668, %t977
%t1671 = fsub fast float %t1662, %t1670
  ; let nvy0 = %t1671
  %il1681 = load float, float* @m1, align 4
%t1682 = fmul fast float %t581, %il1681
%t1684 = fmul fast float %t1682, %t671
%t1685 = fsub fast float %phi_vz_e0, %t1684
  %il1690 = load float, float* @m2, align 4
%t1691 = fmul fast float %t683, %il1690
%t1693 = fmul fast float %t1691, %t773
%t1694 = fsub fast float %t1685, %t1693
  %il1699 = load float, float* @m3, align 4
%t1700 = fmul fast float %t785, %il1699
%t1702 = fmul fast float %t1700, %t875
%t1703 = fsub fast float %t1694, %t1702
  %il1708 = load float, float* @m4, align 4
%t1709 = fmul fast float %t887, %il1708
%t1711 = fmul fast float %t1709, %t977
%t1712 = fsub fast float %t1703, %t1711
  ; let nvz0 = %t1712
  %il1722 = load float, float* @m0, align 4
%t1723 = fmul fast float %t573, %il1722
%t1725 = fmul fast float %t1723, %t671
%t1726 = fadd fast float %phi_vx_e1, %t1725
  %il1731 = load float, float* @m2, align 4
%t1732 = fmul fast float %t981, %il1731
%t1734 = fmul fast float %t1732, %t1079
%t1735 = fsub fast float %t1726, %t1734
  %il1740 = load float, float* @m3, align 4
%t1741 = fmul fast float %t1083, %il1740
%t1743 = fmul fast float %t1741, %t1181
%t1744 = fsub fast float %t1735, %t1743
  %il1749 = load float, float* @m4, align 4
%t1750 = fmul fast float %t1185, %il1749
%t1752 = fmul fast float %t1750, %t1283
%t1753 = fsub fast float %t1744, %t1752
  ; let nvx1 = %t1753
  %il1763 = load float, float* @m0, align 4
%t1764 = fmul fast float %t577, %il1763
%t1766 = fmul fast float %t1764, %t671
%t1767 = fadd fast float %phi_vy_e1, %t1766
  %il1772 = load float, float* @m2, align 4
%t1773 = fmul fast float %t985, %il1772
%t1775 = fmul fast float %t1773, %t1079
%t1776 = fsub fast float %t1767, %t1775
  %il1781 = load float, float* @m3, align 4
%t1782 = fmul fast float %t1087, %il1781
%t1784 = fmul fast float %t1782, %t1181
%t1785 = fsub fast float %t1776, %t1784
  %il1790 = load float, float* @m4, align 4
%t1791 = fmul fast float %t1189, %il1790
%t1793 = fmul fast float %t1791, %t1283
%t1794 = fsub fast float %t1785, %t1793
  ; let nvy1 = %t1794
  %il1804 = load float, float* @m0, align 4
%t1805 = fmul fast float %t581, %il1804
%t1807 = fmul fast float %t1805, %t671
%t1808 = fadd fast float %phi_vz_e1, %t1807
  %il1813 = load float, float* @m2, align 4
%t1814 = fmul fast float %t989, %il1813
%t1816 = fmul fast float %t1814, %t1079
%t1817 = fsub fast float %t1808, %t1816
  %il1822 = load float, float* @m3, align 4
%t1823 = fmul fast float %t1091, %il1822
%t1825 = fmul fast float %t1823, %t1181
%t1826 = fsub fast float %t1817, %t1825
  %il1831 = load float, float* @m4, align 4
%t1832 = fmul fast float %t1193, %il1831
%t1834 = fmul fast float %t1832, %t1283
%t1835 = fsub fast float %t1826, %t1834
  ; let nvz1 = %t1835
  %il1845 = load float, float* @m0, align 4
%t1846 = fmul fast float %t675, %il1845
%t1848 = fmul fast float %t1846, %t773
%t1849 = fadd fast float %phi_vx_e2, %t1848
  %il1854 = load float, float* @m1, align 4
%t1855 = fmul fast float %t981, %il1854
%t1857 = fmul fast float %t1855, %t1079
%t1858 = fadd fast float %t1849, %t1857
  %il1863 = load float, float* @m3, align 4
%t1864 = fmul fast float %t1287, %il1863
%t1866 = fmul fast float %t1864, %t1385
%t1867 = fsub fast float %t1858, %t1866
  %il1872 = load float, float* @m4, align 4
%t1873 = fmul fast float %t1389, %il1872
%t1875 = fmul fast float %t1873, %t1487
%t1876 = fsub fast float %t1867, %t1875
  ; let nvx2 = %t1876
  %il1886 = load float, float* @m0, align 4
%t1887 = fmul fast float %t679, %il1886
%t1889 = fmul fast float %t1887, %t773
%t1890 = fadd fast float %phi_vy_e2, %t1889
  %il1895 = load float, float* @m1, align 4
%t1896 = fmul fast float %t985, %il1895
%t1898 = fmul fast float %t1896, %t1079
%t1899 = fadd fast float %t1890, %t1898
  %il1904 = load float, float* @m3, align 4
%t1905 = fmul fast float %t1291, %il1904
%t1907 = fmul fast float %t1905, %t1385
%t1908 = fsub fast float %t1899, %t1907
  %il1913 = load float, float* @m4, align 4
%t1914 = fmul fast float %t1393, %il1913
%t1916 = fmul fast float %t1914, %t1487
%t1917 = fsub fast float %t1908, %t1916
  ; let nvy2 = %t1917
  %il1927 = load float, float* @m0, align 4
%t1928 = fmul fast float %t683, %il1927
%t1930 = fmul fast float %t1928, %t773
%t1931 = fadd fast float %phi_vz_e2, %t1930
  %il1936 = load float, float* @m1, align 4
%t1937 = fmul fast float %t989, %il1936
%t1939 = fmul fast float %t1937, %t1079
%t1940 = fadd fast float %t1931, %t1939
  %il1945 = load float, float* @m3, align 4
%t1946 = fmul fast float %t1295, %il1945
%t1948 = fmul fast float %t1946, %t1385
%t1949 = fsub fast float %t1940, %t1948
  %il1954 = load float, float* @m4, align 4
%t1955 = fmul fast float %t1397, %il1954
%t1957 = fmul fast float %t1955, %t1487
%t1958 = fsub fast float %t1949, %t1957
  ; let nvz2 = %t1958
  %il1968 = load float, float* @m0, align 4
%t1969 = fmul fast float %t777, %il1968
%t1971 = fmul fast float %t1969, %t875
%t1972 = fadd fast float %phi_vx_e3, %t1971
  %il1977 = load float, float* @m1, align 4
%t1978 = fmul fast float %t1083, %il1977
%t1980 = fmul fast float %t1978, %t1181
%t1981 = fadd fast float %t1972, %t1980
  %il1986 = load float, float* @m2, align 4
%t1987 = fmul fast float %t1287, %il1986
%t1989 = fmul fast float %t1987, %t1385
%t1990 = fadd fast float %t1981, %t1989
  %il1995 = load float, float* @m4, align 4
%t1996 = fmul fast float %t1491, %il1995
%t1998 = fmul fast float %t1996, %t1589
%t1999 = fsub fast float %t1990, %t1998
  ; let nvx3 = %t1999
  %il2009 = load float, float* @m0, align 4
%t2010 = fmul fast float %t781, %il2009
%t2012 = fmul fast float %t2010, %t875
%t2013 = fadd fast float %phi_vy_e3, %t2012
  %il2018 = load float, float* @m1, align 4
%t2019 = fmul fast float %t1087, %il2018
%t2021 = fmul fast float %t2019, %t1181
%t2022 = fadd fast float %t2013, %t2021
  %il2027 = load float, float* @m2, align 4
%t2028 = fmul fast float %t1291, %il2027
%t2030 = fmul fast float %t2028, %t1385
%t2031 = fadd fast float %t2022, %t2030
  %il2036 = load float, float* @m4, align 4
%t2037 = fmul fast float %t1495, %il2036
%t2039 = fmul fast float %t2037, %t1589
%t2040 = fsub fast float %t2031, %t2039
  ; let nvy3 = %t2040
  %il2050 = load float, float* @m0, align 4
%t2051 = fmul fast float %t785, %il2050
%t2053 = fmul fast float %t2051, %t875
%t2054 = fadd fast float %phi_vz_e3, %t2053
  %il2059 = load float, float* @m1, align 4
%t2060 = fmul fast float %t1091, %il2059
%t2062 = fmul fast float %t2060, %t1181
%t2063 = fadd fast float %t2054, %t2062
  %il2068 = load float, float* @m2, align 4
%t2069 = fmul fast float %t1295, %il2068
%t2071 = fmul fast float %t2069, %t1385
%t2072 = fadd fast float %t2063, %t2071
  %il2077 = load float, float* @m4, align 4
%t2078 = fmul fast float %t1499, %il2077
%t2080 = fmul fast float %t2078, %t1589
%t2081 = fsub fast float %t2072, %t2080
  ; let nvz3 = %t2081
  %il2091 = load float, float* @m0, align 4
%t2092 = fmul fast float %t879, %il2091
%t2094 = fmul fast float %t2092, %t977
%t2095 = fadd fast float %phi_vx4, %t2094
  %il2100 = load float, float* @m1, align 4
%t2101 = fmul fast float %t1185, %il2100
%t2103 = fmul fast float %t2101, %t1283
%t2104 = fadd fast float %t2095, %t2103
  %il2109 = load float, float* @m2, align 4
%t2110 = fmul fast float %t1389, %il2109
%t2112 = fmul fast float %t2110, %t1487
%t2113 = fadd fast float %t2104, %t2112
  %il2118 = load float, float* @m3, align 4
%t2119 = fmul fast float %t1491, %il2118
%t2121 = fmul fast float %t2119, %t1589
%t2122 = fadd fast float %t2113, %t2121
  ; let nvx4 = %t2122
  %il2132 = load float, float* @m0, align 4
%t2133 = fmul fast float %t883, %il2132
%t2135 = fmul fast float %t2133, %t977
%t2136 = fadd fast float %phi_vy4, %t2135
  %il2141 = load float, float* @m1, align 4
%t2142 = fmul fast float %t1189, %il2141
%t2144 = fmul fast float %t2142, %t1283
%t2145 = fadd fast float %t2136, %t2144
  %il2150 = load float, float* @m2, align 4
%t2151 = fmul fast float %t1393, %il2150
%t2153 = fmul fast float %t2151, %t1487
%t2154 = fadd fast float %t2145, %t2153
  %il2159 = load float, float* @m3, align 4
%t2160 = fmul fast float %t1495, %il2159
%t2162 = fmul fast float %t2160, %t1589
%t2163 = fadd fast float %t2154, %t2162
  ; let nvy4 = %t2163
  %il2173 = load float, float* @m0, align 4
%t2174 = fmul fast float %t887, %il2173
%t2176 = fmul fast float %t2174, %t977
%t2177 = fadd fast float %phi_vz4, %t2176
  %il2182 = load float, float* @m1, align 4
%t2183 = fmul fast float %t1193, %il2182
%t2185 = fmul fast float %t2183, %t1283
%t2186 = fadd fast float %t2177, %t2185
  %il2191 = load float, float* @m2, align 4
%t2192 = fmul fast float %t1397, %il2191
%t2194 = fmul fast float %t2192, %t1487
%t2195 = fadd fast float %t2186, %t2194
  %il2200 = load float, float* @m3, align 4
%t2201 = fmul fast float %t1499, %il2200
%t2203 = fmul fast float %t2201, %t1589
%t2204 = fadd fast float %t2195, %t2203
  ; let nvz4 = %t2204
   %iv2206_phi_vx_v4 = insertelement <4 x float> %phi_vx_v4, float %t1630, i32 0
   %iv2208_phi_vy_v4 = insertelement <4 x float> %phi_vy_v4, float %t1671, i32 0
   %iv2210_phi_vz_v4 = insertelement <4 x float> %phi_vz_v4, float %t1712, i32 0
   %iv2212_phi_vx_v4 = insertelement <4 x float> %iv2206_phi_vx_v4, float %t1753, i32 1
   %iv2214_phi_vy_v4 = insertelement <4 x float> %iv2208_phi_vy_v4, float %t1794, i32 1
   %iv2216_phi_vz_v4 = insertelement <4 x float> %iv2210_phi_vz_v4, float %t1835, i32 1
   %iv2218_phi_vx_v4 = insertelement <4 x float> %iv2212_phi_vx_v4, float %t1876, i32 2
   %iv2220_phi_vy_v4 = insertelement <4 x float> %iv2214_phi_vy_v4, float %t1917, i32 2
   %iv2222_phi_vz_v4 = insertelement <4 x float> %iv2216_phi_vz_v4, float %t1958, i32 2
   %iv2224_phi_vx_v4 = insertelement <4 x float> %iv2218_phi_vx_v4, float %t1999, i32 3
   %iv2226_phi_vy_v4 = insertelement <4 x float> %iv2220_phi_vy_v4, float %t2040, i32 3
   %iv2228_phi_vz_v4 = insertelement <4 x float> %iv2222_phi_vz_v4, float %t2081, i32 3
  %ap_2230 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 14
  %ap_2232 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 0
  %ap_2234 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 1
  %il2239 = load float, float* @dt, align 4
%t2241 = fmul fast float %il2239, %t1630
%t2242 = fadd fast float %phi_bx_e0, %t2241
   %iv2243_phi_bx_v4 = insertelement <4 x float> %phi_bx_v4, float %t2242, i32 0
  %il2248 = load float, float* @dt, align 4
%t2250 = fmul fast float %il2248, %t1671
%t2251 = fadd fast float %phi_by_e0, %t2250
   %iv2252_phi_by_v4 = insertelement <4 x float> %phi_by_v4, float %t2251, i32 0
  %il2257 = load float, float* @dt, align 4
%t2259 = fmul fast float %il2257, %t1712
%t2260 = fadd fast float %phi_bz_e0, %t2259
   %iv2261_phi_bz_v4 = insertelement <4 x float> %phi_bz_v4, float %t2260, i32 0
  %il2266 = load float, float* @dt, align 4
%t2268 = fmul fast float %il2266, %t1753
%t2269 = fadd fast float %phi_bx_e1, %t2268
   %iv2270_phi_bx_v4 = insertelement <4 x float> %iv2243_phi_bx_v4, float %t2269, i32 1
  %il2275 = load float, float* @dt, align 4
%t2277 = fmul fast float %il2275, %t1794
%t2278 = fadd fast float %phi_by_e1, %t2277
   %iv2279_phi_by_v4 = insertelement <4 x float> %iv2252_phi_by_v4, float %t2278, i32 1
  %il2284 = load float, float* @dt, align 4
%t2286 = fmul fast float %il2284, %t1835
%t2287 = fadd fast float %phi_bz_e1, %t2286
   %iv2288_phi_bz_v4 = insertelement <4 x float> %iv2261_phi_bz_v4, float %t2287, i32 1
  %il2293 = load float, float* @dt, align 4
%t2295 = fmul fast float %il2293, %t1876
%t2296 = fadd fast float %phi_bx_e2, %t2295
   %iv2297_phi_bx_v4 = insertelement <4 x float> %iv2270_phi_bx_v4, float %t2296, i32 2
  %il2302 = load float, float* @dt, align 4
%t2304 = fmul fast float %il2302, %t1917
%t2305 = fadd fast float %phi_by_e2, %t2304
   %iv2306_phi_by_v4 = insertelement <4 x float> %iv2279_phi_by_v4, float %t2305, i32 2
  %il2311 = load float, float* @dt, align 4
%t2313 = fmul fast float %il2311, %t1958
%t2314 = fadd fast float %phi_bz_e2, %t2313
   %iv2315_phi_bz_v4 = insertelement <4 x float> %iv2288_phi_bz_v4, float %t2314, i32 2
  %il2320 = load float, float* @dt, align 4
%t2322 = fmul fast float %il2320, %t1999
%t2323 = fadd fast float %phi_bx_e3, %t2322
   %iv2324_phi_bx_v4 = insertelement <4 x float> %iv2297_phi_bx_v4, float %t2323, i32 3
  %il2329 = load float, float* @dt, align 4
%t2331 = fmul fast float %il2329, %t2040
%t2332 = fadd fast float %phi_by_e3, %t2331
   %iv2333_phi_by_v4 = insertelement <4 x float> %iv2306_phi_by_v4, float %t2332, i32 3
  %il2338 = load float, float* @dt, align 4
%t2340 = fmul fast float %il2338, %t2081
%t2341 = fadd fast float %phi_bz_e3, %t2340
   %iv2342_phi_bz_v4 = insertelement <4 x float> %iv2315_phi_bz_v4, float %t2341, i32 3
  %il2347 = load float, float* @dt, align 4
%t2349 = fmul fast float %il2347, %t2122
%t2350 = fadd fast float %phi_bx4, %t2349
  %ap_2351 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 11
  %il2356 = load float, float* @dt, align 4
%t2358 = fmul fast float %il2356, %t2163
%t2359 = fadd fast float %phi_by4, %t2358
  %ap_2360 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 12
  %il2365 = load float, float* @dt, align 4
%t2367 = fmul fast float %il2365, %t2204
%t2368 = fadd fast float %phi_bz4, %t2367
  %ap_2369 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 13
  %t2371 = add i64 0, %phi_count
%t2373 = add i64 0, 1
%t2374 = add nsw i64 %t2371, %t2373
  %ap_2375 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
  store i64 %t2374, ptr %ap_2375, align 8, !tbaa !1
%t2379 = fsub fast float %phi_bx_e0, %phi_bx_e1
  ; let dxe01 = %t2379
%t2383 = fsub fast float %phi_by_e0, %phi_by_e1
  ; let dye01 = %t2383
%t2387 = fsub fast float %phi_bz_e0, %phi_bz_e1
  ; let dze01 = %t2387
%t2393 = fmul fast float %t2379, %t2379
%t2397 = fmul fast float %t2383, %t2383
%t2398 = fadd fast float %t2393, %t2397
%t2402 = fmul fast float %t2387, %t2387
%t2403 = fadd fast float %t2398, %t2402
  ; let dsqe01 = %t2403
%ff2408 = bitcast i32 1056964608 to float
%t2409 = fmul fast float %t2403, %ff2408
  ; let gea01 = %t2409
%ff2413 = bitcast i32 1056964608 to float
%t2419 = fdiv fast float %t2403, %t2409
%t2420 = fadd fast float %t2409, %t2419
%t2421 = fmul fast float %ff2413, %t2420
  ; let geb01 = %t2421
%ff2425 = bitcast i32 1056964608 to float
%t2431 = fdiv fast float %t2403, %t2421
%t2432 = fadd fast float %t2421, %t2431
%t2433 = fmul fast float %ff2425, %t2432
  ; let gec01 = %t2433
%ff2437 = bitcast i32 1056964608 to float
%t2443 = fdiv fast float %t2403, %t2433
%t2444 = fadd fast float %t2433, %t2443
%t2445 = fmul fast float %ff2437, %t2444
  ; let ged01 = %t2445
%ff2449 = bitcast i32 1056964608 to float
%t2455 = fdiv fast float %t2403, %t2445
%t2456 = fadd fast float %t2445, %t2455
%t2457 = fmul fast float %ff2449, %t2456
  ; let gee01 = %t2457
%ff2461 = bitcast i32 1056964608 to float
%t2467 = fdiv fast float %t2403, %t2457
%t2468 = fadd fast float %t2457, %t2467
%t2469 = fmul fast float %ff2461, %t2468
  ; let edist01 = %t2469
%t2473 = fsub fast float %phi_bx_e0, %phi_bx_e2
  ; let dxe02 = %t2473
%t2477 = fsub fast float %phi_by_e0, %phi_by_e2
  ; let dye02 = %t2477
%t2481 = fsub fast float %phi_bz_e0, %phi_bz_e2
  ; let dze02 = %t2481
%t2487 = fmul fast float %t2473, %t2473
%t2491 = fmul fast float %t2477, %t2477
%t2492 = fadd fast float %t2487, %t2491
%t2496 = fmul fast float %t2481, %t2481
%t2497 = fadd fast float %t2492, %t2496
  ; let dsqe02 = %t2497
%ff2502 = bitcast i32 1056964608 to float
%t2503 = fmul fast float %t2497, %ff2502
  ; let gea02 = %t2503
%ff2507 = bitcast i32 1056964608 to float
%t2513 = fdiv fast float %t2497, %t2503
%t2514 = fadd fast float %t2503, %t2513
%t2515 = fmul fast float %ff2507, %t2514
  ; let geb02 = %t2515
%ff2519 = bitcast i32 1056964608 to float
%t2525 = fdiv fast float %t2497, %t2515
%t2526 = fadd fast float %t2515, %t2525
%t2527 = fmul fast float %ff2519, %t2526
  ; let gec02 = %t2527
%ff2531 = bitcast i32 1056964608 to float
%t2537 = fdiv fast float %t2497, %t2527
%t2538 = fadd fast float %t2527, %t2537
%t2539 = fmul fast float %ff2531, %t2538
  ; let ged02 = %t2539
%ff2543 = bitcast i32 1056964608 to float
%t2549 = fdiv fast float %t2497, %t2539
%t2550 = fadd fast float %t2539, %t2549
%t2551 = fmul fast float %ff2543, %t2550
  ; let gee02 = %t2551
%ff2555 = bitcast i32 1056964608 to float
%t2561 = fdiv fast float %t2497, %t2551
%t2562 = fadd fast float %t2551, %t2561
%t2563 = fmul fast float %ff2555, %t2562
  ; let edist02 = %t2563
%t2567 = fsub fast float %phi_bx_e0, %phi_bx_e3
  ; let dxe03 = %t2567
%t2571 = fsub fast float %phi_by_e0, %phi_by_e3
  ; let dye03 = %t2571
%t2575 = fsub fast float %phi_bz_e0, %phi_bz_e3
  ; let dze03 = %t2575
%t2581 = fmul fast float %t2567, %t2567
%t2585 = fmul fast float %t2571, %t2571
%t2586 = fadd fast float %t2581, %t2585
%t2590 = fmul fast float %t2575, %t2575
%t2591 = fadd fast float %t2586, %t2590
  ; let dsqe03 = %t2591
%ff2596 = bitcast i32 1056964608 to float
%t2597 = fmul fast float %t2591, %ff2596
  ; let gea03 = %t2597
%ff2601 = bitcast i32 1056964608 to float
%t2607 = fdiv fast float %t2591, %t2597
%t2608 = fadd fast float %t2597, %t2607
%t2609 = fmul fast float %ff2601, %t2608
  ; let geb03 = %t2609
%ff2613 = bitcast i32 1056964608 to float
%t2619 = fdiv fast float %t2591, %t2609
%t2620 = fadd fast float %t2609, %t2619
%t2621 = fmul fast float %ff2613, %t2620
  ; let gec03 = %t2621
%ff2625 = bitcast i32 1056964608 to float
%t2631 = fdiv fast float %t2591, %t2621
%t2632 = fadd fast float %t2621, %t2631
%t2633 = fmul fast float %ff2625, %t2632
  ; let ged03 = %t2633
%ff2637 = bitcast i32 1056964608 to float
%t2643 = fdiv fast float %t2591, %t2633
%t2644 = fadd fast float %t2633, %t2643
%t2645 = fmul fast float %ff2637, %t2644
  ; let gee03 = %t2645
%ff2649 = bitcast i32 1056964608 to float
%t2655 = fdiv fast float %t2591, %t2645
%t2656 = fadd fast float %t2645, %t2655
%t2657 = fmul fast float %ff2649, %t2656
  ; let edist03 = %t2657
%t2661 = fsub fast float %phi_bx_e0, %phi_bx4
  ; let dxe04 = %t2661
%t2665 = fsub fast float %phi_by_e0, %phi_by4
  ; let dye04 = %t2665
%t2669 = fsub fast float %phi_bz_e0, %phi_bz4
  ; let dze04 = %t2669
%t2675 = fmul fast float %t2661, %t2661
%t2679 = fmul fast float %t2665, %t2665
%t2680 = fadd fast float %t2675, %t2679
%t2684 = fmul fast float %t2669, %t2669
%t2685 = fadd fast float %t2680, %t2684
  ; let dsqe04 = %t2685
%ff2690 = bitcast i32 1056964608 to float
%t2691 = fmul fast float %t2685, %ff2690
  ; let gea04 = %t2691
%ff2695 = bitcast i32 1056964608 to float
%t2701 = fdiv fast float %t2685, %t2691
%t2702 = fadd fast float %t2691, %t2701
%t2703 = fmul fast float %ff2695, %t2702
  ; let geb04 = %t2703
%ff2707 = bitcast i32 1056964608 to float
%t2713 = fdiv fast float %t2685, %t2703
%t2714 = fadd fast float %t2703, %t2713
%t2715 = fmul fast float %ff2707, %t2714
  ; let gec04 = %t2715
%ff2719 = bitcast i32 1056964608 to float
%t2725 = fdiv fast float %t2685, %t2715
%t2726 = fadd fast float %t2715, %t2725
%t2727 = fmul fast float %ff2719, %t2726
  ; let ged04 = %t2727
%ff2731 = bitcast i32 1056964608 to float
%t2737 = fdiv fast float %t2685, %t2727
%t2738 = fadd fast float %t2727, %t2737
%t2739 = fmul fast float %ff2731, %t2738
  ; let gee04 = %t2739
%ff2743 = bitcast i32 1056964608 to float
%t2749 = fdiv fast float %t2685, %t2739
%t2750 = fadd fast float %t2739, %t2749
%t2751 = fmul fast float %ff2743, %t2750
  ; let edist04 = %t2751
%t2755 = fsub fast float %phi_bx_e1, %phi_bx_e2
  ; let dxe12 = %t2755
%t2759 = fsub fast float %phi_by_e1, %phi_by_e2
  ; let dye12 = %t2759
%t2763 = fsub fast float %phi_bz_e1, %phi_bz_e2
  ; let dze12 = %t2763
%t2769 = fmul fast float %t2755, %t2755
%t2773 = fmul fast float %t2759, %t2759
%t2774 = fadd fast float %t2769, %t2773
%t2778 = fmul fast float %t2763, %t2763
%t2779 = fadd fast float %t2774, %t2778
  ; let dsqe12 = %t2779
%ff2784 = bitcast i32 1056964608 to float
%t2785 = fmul fast float %t2779, %ff2784
  ; let gea12 = %t2785
%ff2789 = bitcast i32 1056964608 to float
%t2795 = fdiv fast float %t2779, %t2785
%t2796 = fadd fast float %t2785, %t2795
%t2797 = fmul fast float %ff2789, %t2796
  ; let geb12 = %t2797
%ff2801 = bitcast i32 1056964608 to float
%t2807 = fdiv fast float %t2779, %t2797
%t2808 = fadd fast float %t2797, %t2807
%t2809 = fmul fast float %ff2801, %t2808
  ; let gec12 = %t2809
%ff2813 = bitcast i32 1056964608 to float
%t2819 = fdiv fast float %t2779, %t2809
%t2820 = fadd fast float %t2809, %t2819
%t2821 = fmul fast float %ff2813, %t2820
  ; let ged12 = %t2821
%ff2825 = bitcast i32 1056964608 to float
%t2831 = fdiv fast float %t2779, %t2821
%t2832 = fadd fast float %t2821, %t2831
%t2833 = fmul fast float %ff2825, %t2832
  ; let gee12 = %t2833
%ff2837 = bitcast i32 1056964608 to float
%t2843 = fdiv fast float %t2779, %t2833
%t2844 = fadd fast float %t2833, %t2843
%t2845 = fmul fast float %ff2837, %t2844
  ; let edist12 = %t2845
%t2849 = fsub fast float %phi_bx_e1, %phi_bx_e3
  ; let dxe13 = %t2849
%t2853 = fsub fast float %phi_by_e1, %phi_by_e3
  ; let dye13 = %t2853
%t2857 = fsub fast float %phi_bz_e1, %phi_bz_e3
  ; let dze13 = %t2857
%t2863 = fmul fast float %t2849, %t2849
%t2867 = fmul fast float %t2853, %t2853
%t2868 = fadd fast float %t2863, %t2867
%t2872 = fmul fast float %t2857, %t2857
%t2873 = fadd fast float %t2868, %t2872
  ; let dsqe13 = %t2873
%ff2878 = bitcast i32 1056964608 to float
%t2879 = fmul fast float %t2873, %ff2878
  ; let gea13 = %t2879
%ff2883 = bitcast i32 1056964608 to float
%t2889 = fdiv fast float %t2873, %t2879
%t2890 = fadd fast float %t2879, %t2889
%t2891 = fmul fast float %ff2883, %t2890
  ; let geb13 = %t2891
%ff2895 = bitcast i32 1056964608 to float
%t2901 = fdiv fast float %t2873, %t2891
%t2902 = fadd fast float %t2891, %t2901
%t2903 = fmul fast float %ff2895, %t2902
  ; let gec13 = %t2903
%ff2907 = bitcast i32 1056964608 to float
%t2913 = fdiv fast float %t2873, %t2903
%t2914 = fadd fast float %t2903, %t2913
%t2915 = fmul fast float %ff2907, %t2914
  ; let ged13 = %t2915
%ff2919 = bitcast i32 1056964608 to float
%t2925 = fdiv fast float %t2873, %t2915
%t2926 = fadd fast float %t2915, %t2925
%t2927 = fmul fast float %ff2919, %t2926
  ; let gee13 = %t2927
%ff2931 = bitcast i32 1056964608 to float
%t2937 = fdiv fast float %t2873, %t2927
%t2938 = fadd fast float %t2927, %t2937
%t2939 = fmul fast float %ff2931, %t2938
  ; let edist13 = %t2939
%t2943 = fsub fast float %phi_bx_e1, %phi_bx4
  ; let dxe14 = %t2943
%t2947 = fsub fast float %phi_by_e1, %phi_by4
  ; let dye14 = %t2947
%t2951 = fsub fast float %phi_bz_e1, %phi_bz4
  ; let dze14 = %t2951
%t2957 = fmul fast float %t2943, %t2943
%t2961 = fmul fast float %t2947, %t2947
%t2962 = fadd fast float %t2957, %t2961
%t2966 = fmul fast float %t2951, %t2951
%t2967 = fadd fast float %t2962, %t2966
  ; let dsqe14 = %t2967
%ff2972 = bitcast i32 1056964608 to float
%t2973 = fmul fast float %t2967, %ff2972
  ; let gea14 = %t2973
%ff2977 = bitcast i32 1056964608 to float
%t2983 = fdiv fast float %t2967, %t2973
%t2984 = fadd fast float %t2973, %t2983
%t2985 = fmul fast float %ff2977, %t2984
  ; let geb14 = %t2985
%ff2989 = bitcast i32 1056964608 to float
%t2995 = fdiv fast float %t2967, %t2985
%t2996 = fadd fast float %t2985, %t2995
%t2997 = fmul fast float %ff2989, %t2996
  ; let gec14 = %t2997
%ff3001 = bitcast i32 1056964608 to float
%t3007 = fdiv fast float %t2967, %t2997
%t3008 = fadd fast float %t2997, %t3007
%t3009 = fmul fast float %ff3001, %t3008
  ; let ged14 = %t3009
%ff3013 = bitcast i32 1056964608 to float
%t3019 = fdiv fast float %t2967, %t3009
%t3020 = fadd fast float %t3009, %t3019
%t3021 = fmul fast float %ff3013, %t3020
  ; let gee14 = %t3021
%ff3025 = bitcast i32 1056964608 to float
%t3031 = fdiv fast float %t2967, %t3021
%t3032 = fadd fast float %t3021, %t3031
%t3033 = fmul fast float %ff3025, %t3032
  ; let edist14 = %t3033
%t3037 = fsub fast float %phi_bx_e2, %phi_bx_e3
  ; let dxe23 = %t3037
%t3041 = fsub fast float %phi_by_e2, %phi_by_e3
  ; let dye23 = %t3041
%t3045 = fsub fast float %phi_bz_e2, %phi_bz_e3
  ; let dze23 = %t3045
%t3051 = fmul fast float %t3037, %t3037
%t3055 = fmul fast float %t3041, %t3041
%t3056 = fadd fast float %t3051, %t3055
%t3060 = fmul fast float %t3045, %t3045
%t3061 = fadd fast float %t3056, %t3060
  ; let dsqe23 = %t3061
%ff3066 = bitcast i32 1056964608 to float
%t3067 = fmul fast float %t3061, %ff3066
  ; let gea23 = %t3067
%ff3071 = bitcast i32 1056964608 to float
%t3077 = fdiv fast float %t3061, %t3067
%t3078 = fadd fast float %t3067, %t3077
%t3079 = fmul fast float %ff3071, %t3078
  ; let geb23 = %t3079
%ff3083 = bitcast i32 1056964608 to float
%t3089 = fdiv fast float %t3061, %t3079
%t3090 = fadd fast float %t3079, %t3089
%t3091 = fmul fast float %ff3083, %t3090
  ; let gec23 = %t3091
%ff3095 = bitcast i32 1056964608 to float
%t3101 = fdiv fast float %t3061, %t3091
%t3102 = fadd fast float %t3091, %t3101
%t3103 = fmul fast float %ff3095, %t3102
  ; let ged23 = %t3103
%ff3107 = bitcast i32 1056964608 to float
%t3113 = fdiv fast float %t3061, %t3103
%t3114 = fadd fast float %t3103, %t3113
%t3115 = fmul fast float %ff3107, %t3114
  ; let gee23 = %t3115
%ff3119 = bitcast i32 1056964608 to float
%t3125 = fdiv fast float %t3061, %t3115
%t3126 = fadd fast float %t3115, %t3125
%t3127 = fmul fast float %ff3119, %t3126
  ; let edist23 = %t3127
%t3131 = fsub fast float %phi_bx_e2, %phi_bx4
  ; let dxe24 = %t3131
%t3135 = fsub fast float %phi_by_e2, %phi_by4
  ; let dye24 = %t3135
%t3139 = fsub fast float %phi_bz_e2, %phi_bz4
  ; let dze24 = %t3139
%t3145 = fmul fast float %t3131, %t3131
%t3149 = fmul fast float %t3135, %t3135
%t3150 = fadd fast float %t3145, %t3149
%t3154 = fmul fast float %t3139, %t3139
%t3155 = fadd fast float %t3150, %t3154
  ; let dsqe24 = %t3155
%ff3160 = bitcast i32 1056964608 to float
%t3161 = fmul fast float %t3155, %ff3160
  ; let gea24 = %t3161
%ff3165 = bitcast i32 1056964608 to float
%t3171 = fdiv fast float %t3155, %t3161
%t3172 = fadd fast float %t3161, %t3171
%t3173 = fmul fast float %ff3165, %t3172
  ; let geb24 = %t3173
%ff3177 = bitcast i32 1056964608 to float
%t3183 = fdiv fast float %t3155, %t3173
%t3184 = fadd fast float %t3173, %t3183
%t3185 = fmul fast float %ff3177, %t3184
  ; let gec24 = %t3185
%ff3189 = bitcast i32 1056964608 to float
%t3195 = fdiv fast float %t3155, %t3185
%t3196 = fadd fast float %t3185, %t3195
%t3197 = fmul fast float %ff3189, %t3196
  ; let ged24 = %t3197
%ff3201 = bitcast i32 1056964608 to float
%t3207 = fdiv fast float %t3155, %t3197
%t3208 = fadd fast float %t3197, %t3207
%t3209 = fmul fast float %ff3201, %t3208
  ; let gee24 = %t3209
%ff3213 = bitcast i32 1056964608 to float
%t3219 = fdiv fast float %t3155, %t3209
%t3220 = fadd fast float %t3209, %t3219
%t3221 = fmul fast float %ff3213, %t3220
  ; let edist24 = %t3221
%t3225 = fsub fast float %phi_bx_e3, %phi_bx4
  ; let dxe34 = %t3225
%t3229 = fsub fast float %phi_by_e3, %phi_by4
  ; let dye34 = %t3229
%t3233 = fsub fast float %phi_bz_e3, %phi_bz4
  ; let dze34 = %t3233
%t3239 = fmul fast float %t3225, %t3225
%t3243 = fmul fast float %t3229, %t3229
%t3244 = fadd fast float %t3239, %t3243
%t3248 = fmul fast float %t3233, %t3233
%t3249 = fadd fast float %t3244, %t3248
  ; let dsqe34 = %t3249
%ff3254 = bitcast i32 1056964608 to float
%t3255 = fmul fast float %t3249, %ff3254
  ; let gea34 = %t3255
%ff3259 = bitcast i32 1056964608 to float
%t3265 = fdiv fast float %t3249, %t3255
%t3266 = fadd fast float %t3255, %t3265
%t3267 = fmul fast float %ff3259, %t3266
  ; let geb34 = %t3267
%ff3271 = bitcast i32 1056964608 to float
%t3277 = fdiv fast float %t3249, %t3267
%t3278 = fadd fast float %t3267, %t3277
%t3279 = fmul fast float %ff3271, %t3278
  ; let gec34 = %t3279
%ff3283 = bitcast i32 1056964608 to float
%t3289 = fdiv fast float %t3249, %t3279
%t3290 = fadd fast float %t3279, %t3289
%t3291 = fmul fast float %ff3283, %t3290
  ; let ged34 = %t3291
%ff3295 = bitcast i32 1056964608 to float
%t3301 = fdiv fast float %t3249, %t3291
%t3302 = fadd fast float %t3291, %t3301
%t3303 = fmul fast float %ff3295, %t3302
  ; let gee34 = %t3303
%ff3307 = bitcast i32 1056964608 to float
%t3313 = fdiv fast float %t3249, %t3303
%t3314 = fadd fast float %t3303, %t3313
%t3315 = fmul fast float %ff3307, %t3314
  ; let edist34 = %t3315
  %il3319 = load float, float* @m0, align 4
  %il3321 = load float, float* @m1, align 4
%t3322 = fmul fast float %il3319, %il3321
%t3324 = fdiv fast float %t3322, %t2469
  ; let epex01 = %t3324
  %il3328 = load float, float* @m0, align 4
  %il3330 = load float, float* @m2, align 4
%t3331 = fmul fast float %il3328, %il3330
%t3333 = fdiv fast float %t3331, %t2563
  ; let epex02 = %t3333
  %il3337 = load float, float* @m0, align 4
  %il3339 = load float, float* @m3, align 4
%t3340 = fmul fast float %il3337, %il3339
%t3342 = fdiv fast float %t3340, %t2657
  ; let epex03 = %t3342
  %il3346 = load float, float* @m0, align 4
  %il3348 = load float, float* @m4, align 4
%t3349 = fmul fast float %il3346, %il3348
%t3351 = fdiv fast float %t3349, %t2751
  ; let epex04 = %t3351
  %il3355 = load float, float* @m1, align 4
  %il3357 = load float, float* @m2, align 4
%t3358 = fmul fast float %il3355, %il3357
%t3360 = fdiv fast float %t3358, %t2845
  ; let epex12 = %t3360
  %il3364 = load float, float* @m1, align 4
  %il3366 = load float, float* @m3, align 4
%t3367 = fmul fast float %il3364, %il3366
%t3369 = fdiv fast float %t3367, %t2939
  ; let epex13 = %t3369
  %il3373 = load float, float* @m1, align 4
  %il3375 = load float, float* @m4, align 4
%t3376 = fmul fast float %il3373, %il3375
%t3378 = fdiv fast float %t3376, %t3033
  ; let epex14 = %t3378
  %il3382 = load float, float* @m2, align 4
  %il3384 = load float, float* @m3, align 4
%t3385 = fmul fast float %il3382, %il3384
%t3387 = fdiv fast float %t3385, %t3127
  ; let epex23 = %t3387
  %il3391 = load float, float* @m2, align 4
  %il3393 = load float, float* @m4, align 4
%t3394 = fmul fast float %il3391, %il3393
%t3396 = fdiv fast float %t3394, %t3221
  ; let epex24 = %t3396
  %il3400 = load float, float* @m3, align 4
  %il3402 = load float, float* @m4, align 4
%t3403 = fmul fast float %il3400, %il3402
%t3405 = fdiv fast float %t3403, %t3315
  ; let epex34 = %t3405
%t3419 = fadd fast float %t3324, %t3333
%t3421 = fadd fast float %t3419, %t3342
%t3423 = fadd fast float %t3421, %t3351
%t3425 = fadd fast float %t3423, %t3360
%t3427 = fadd fast float %t3425, %t3369
%t3429 = fadd fast float %t3427, %t3378
%t3431 = fadd fast float %t3429, %t3387
%t3433 = fadd fast float %t3431, %t3396
%t3435 = fadd fast float %t3433, %t3405
  %t3436 = fneg float %t3435
  ; let epp = %t3436
%ff3441 = bitcast i32 1056964608 to float
  %il3443 = load float, float* @m0, align 4
%t3444 = fmul fast float %ff3441, %il3443
%t3450 = fmul fast float %phi_vx_e0, %phi_vx_e0
%t3454 = fmul fast float %phi_vy_e0, %phi_vy_e0
%t3455 = fadd fast float %t3450, %t3454
%t3459 = fmul fast float %phi_vz_e0, %phi_vz_e0
%t3460 = fadd fast float %t3455, %t3459
%t3461 = fmul fast float %t3444, %t3460
  ; let ek0c = %t3461
%ff3466 = bitcast i32 1056964608 to float
  %il3468 = load float, float* @m1, align 4
%t3469 = fmul fast float %ff3466, %il3468
%t3475 = fmul fast float %phi_vx_e1, %phi_vx_e1
%t3479 = fmul fast float %phi_vy_e1, %phi_vy_e1
%t3480 = fadd fast float %t3475, %t3479
%t3484 = fmul fast float %phi_vz_e1, %phi_vz_e1
%t3485 = fadd fast float %t3480, %t3484
%t3486 = fmul fast float %t3469, %t3485
  ; let ek1c = %t3486
%ff3491 = bitcast i32 1056964608 to float
  %il3493 = load float, float* @m2, align 4
%t3494 = fmul fast float %ff3491, %il3493
%t3500 = fmul fast float %phi_vx_e2, %phi_vx_e2
%t3504 = fmul fast float %phi_vy_e2, %phi_vy_e2
%t3505 = fadd fast float %t3500, %t3504
%t3509 = fmul fast float %phi_vz_e2, %phi_vz_e2
%t3510 = fadd fast float %t3505, %t3509
%t3511 = fmul fast float %t3494, %t3510
  ; let ek2c = %t3511
%ff3516 = bitcast i32 1056964608 to float
  %il3518 = load float, float* @m3, align 4
%t3519 = fmul fast float %ff3516, %il3518
%t3525 = fmul fast float %phi_vx_e3, %phi_vx_e3
%t3529 = fmul fast float %phi_vy_e3, %phi_vy_e3
%t3530 = fadd fast float %t3525, %t3529
%t3534 = fmul fast float %phi_vz_e3, %phi_vz_e3
%t3535 = fadd fast float %t3530, %t3534
%t3536 = fmul fast float %t3519, %t3535
  ; let ek3c = %t3536
%ff3541 = bitcast i32 1056964608 to float
  %il3543 = load float, float* @m4, align 4
%t3544 = fmul fast float %ff3541, %il3543
%t3550 = fmul fast float %phi_vx4, %phi_vx4
%t3554 = fmul fast float %phi_vy4, %phi_vy4
%t3555 = fadd fast float %t3550, %t3554
%t3559 = fmul fast float %phi_vz4, %phi_vz4
%t3560 = fadd fast float %t3555, %t3559
%t3561 = fmul fast float %t3544, %t3560
  ; let ek4c = %t3561
%t3568 = fadd fast float %t3461, %t3486
%t3570 = fadd fast float %t3568, %t3511
%t3572 = fadd fast float %t3570, %t3536
%t3574 = fadd fast float %t3572, %t3561
  ; let ekc = %t3574
%t3578 = fadd fast float %t3436, %t3574
  ; let energy = %t3578
  %ap_3580 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 2
  %t3583 = add i64 0, %t2374
%t3585 = add i64 0, 5000000
%t3586 = srem i64 %t3583, %t3585
%t3588 = add i64 0, 0
%c3589 = icmp eq i64 %t3586, %t3588
  br i1 %c3589, label %g3590_t, label %g3590_e
  g3590_t:
    %pfd3593 = fpext float %t3578 to double
    %pso3594 = load volatile ptr, ptr @stdout
    %pff3595 = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0
    %ppf3596 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso3594, ptr %pff3595, double %pfd3593)
    %t3591 = zext i32 %ppf3596 to i64
    ; let __periodic = %t3591
    br label %g3590_tx
  g3590_tx:
    br label %g3590_e
  g3590_e:
  br label %latch
latch:
  %pn_cnt_544 = add i64 %pi_cnt_544, 1
  %be_bound = add i64 0, %phi_bound
  %be_bx_v4 = bitcast <4 x float> %iv2324_phi_bx_v4 to <4 x float>
  %be_bx4 = fadd float %t2350, 0.0
  %be_by_v4 = bitcast <4 x float> %iv2333_phi_by_v4 to <4 x float>
  %be_by4 = fadd float %t2359, 0.0
  %be_bz_v4 = bitcast <4 x float> %iv2342_phi_bz_v4 to <4 x float>
  %be_bz4 = fadd float %t2368, 0.0
  %be_cycle_count = add i64 0, %phi_cycle_count
  %be_last_energy = fadd float %t3578, 0.0
  %be_vx_v4 = bitcast <4 x float> %iv2224_phi_vx_v4 to <4 x float>
  %be_vx4 = fadd float %t2122, 0.0
  %be_vy_v4 = bitcast <4 x float> %iv2226_phi_vy_v4 to <4 x float>
  %be_vy4 = fadd float %t2163, 0.0
  %be_vz_v4 = bitcast <4 x float> %iv2228_phi_vz_v4 to <4 x float>
  %be_vz4 = fadd float %t2204, 0.0
  %be_count = add i64 0, %pn_cnt_544
  br label %loop_hdr, !llvm.loop !100
commit:
  store float %phi_last_energy, ptr %lv_475, align 4
  br label %done
done:
  %arr4 = load ptr, ptr %arbase3, align 8
  store i8* %arr4, ptr %arptr3, align 8
  %lv_3597 = load float, ptr %lv_475, align 4
  %pfd3600 = fpext float %lv_3597 to double
  %pso3601 = load volatile ptr, ptr @stdout
  %pff3602 = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0
  %ppf3603 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso3601, ptr %pff3602, double %pfd3600)
  %t3598 = zext i32 %ppf3603 to i64
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
attributes #4 = {
    mustprogress nofree norecurse nosync nounwind memory(readwrite)
    "disable-slp-vectorize"="true" "no-vectorize-slp"="true"
}
attributes #5 = {
    nofree norecurse nosync nounwind memory(readwrite)
    "disable-slp-vectorize"="true" "no-vectorize-slp"="true"
}
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
