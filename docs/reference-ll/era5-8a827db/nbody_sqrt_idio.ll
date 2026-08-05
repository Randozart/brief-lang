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
@m4 = constant float bitcast (i32 990201755 to float)
@dt = constant float bitcast (i32 1008981770 to float)
@m3 = constant float bitcast (i32 987885205 to float)
@m0 = constant float bitcast (i32 1109256678 to float)
@solar_mass = alias float, float* @m0
@m1 = constant float bitcast (i32 1025139887 to float)
@m2 = constant float bitcast (i32 1010362952 to float)
@dpy = constant float bitcast (i32 1136041656 to float)
@pi = constant float bitcast (i32 1078530011 to float)

%StateChunk0 = type { i64, i64, float, float, float, float, float, float, float, float, float, float, float, float, float }
%StateChunk1 = type { float, float, float, float, float, float, float, float, float, float, float, float, float, float, float }
%StateChunk2 = type { float, float, i64 }
%State = type { i64, i64, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, float, i64 }
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
  %r = call i64@briv_open (i64 %path, i64 %flags, i64 %mode);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_close(i64 %fd) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_close (i64 %fd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_read(i64 %fd, i64 %buf, i64 %count) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_read (i64 %fd, i64 %buf, i64 %count);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_write(i64 %fd, i64 %buf, i64 %count) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_write (i64 %fd, i64 %buf, i64 %count);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_lseek(i64 %fd, i64 %offset, i64 %whence) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_lseek (i64 %fd, i64 %offset, i64 %whence);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_pread(i64 %fd, i64 %buf, i64 %count, i64 %offset) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_pread (i64 %fd, i64 %buf, i64 %count, i64 %offset);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_pwrite(i64 %fd, i64 %buf, i64 %count, i64 %offset) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_pwrite (i64 %fd, i64 %buf, i64 %count, i64 %offset);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_stat(i64 %path, i64 %buf) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_stat (i64 %path, i64 %buf);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_fstat(i64 %fd, i64 %buf) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_fstat (i64 %fd, i64 %buf);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_ftruncate(i64 %fd, i64 %length) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_ftruncate (i64 %fd, i64 %length);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_fsync(i64 %fd) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_fsync (i64 %fd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_dup(i64 %fd) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_dup (i64 %fd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_dup2(i64 %oldfd, i64 %newfd) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_dup2 (i64 %oldfd, i64 %newfd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_fcntl(i64 %fd, i64 %cmd, i64 %arg) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_fcntl (i64 %fd, i64 %cmd, i64 %arg);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_socket(i64 %domain, i64 %type_, i64 %protocol) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_socket (i64 %domain, i64 %type_, i64 %protocol);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_bind(i64 %fd, i64 %addr, i64 %addrlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_bind (i64 %fd, i64 %addr, i64 %addrlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_listen(i64 %fd, i64 %backlog) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_listen (i64 %fd, i64 %backlog);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_accept(i64 %fd, i64 %addr, i64 %addrlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_accept (i64 %fd, i64 %addr, i64 %addrlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_connect(i64 %fd, i64 %addr, i64 %addrlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_connect (i64 %fd, i64 %addr, i64 %addrlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_send(i64 %fd, i64 %buf, i64 %len, i64 %flags) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_send (i64 %fd, i64 %buf, i64 %len, i64 %flags);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_recv(i64 %fd, i64 %buf, i64 %len, i64 %flags) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_recv (i64 %fd, i64 %buf, i64 %len, i64 %flags);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_sendto(i64 %fd, i64 %buf, i64 %len, i64 %flags, i64 %dest, i64 %destlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_sendto (i64 %fd, i64 %buf, i64 %len, i64 %flags, i64 %dest, i64 %destlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_recvfrom(i64 %fd, i64 %buf, i64 %len, i64 %flags, i64 %src, i64 %srclen) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_recvfrom (i64 %fd, i64 %buf, i64 %len, i64 %flags, i64 %src, i64 %srclen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_setsockopt(i64 %fd, i64 %level, i64 %optname, i64 %optval, i64 %optlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_setsockopt (i64 %fd, i64 %level, i64 %optname, i64 %optval, i64 %optlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getsockopt(i64 %fd, i64 %level, i64 %optname, i64 %optval, i64 %optlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_getsockopt (i64 %fd, i64 %level, i64 %optname, i64 %optval, i64 %optlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_shutdown(i64 %fd, i64 %how) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_shutdown (i64 %fd, i64 %how);
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
  %r = call i64@briv_pipe (i64 %pipefd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @shm_open(i64 %name, i64 %oflag, i64 %mode) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_shm_open (i64 %name, i64 %oflag, i64 %mode);
  ret i64 %r;
  ret i64 0
}

define internal i64 @shm_unlink(i64 %name) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_shm_unlink (i64 %name);
  ret i64 %r;
  ret i64 0
}

define internal i64 @sem_open(i64 %name, i64 %oflag, i64 %mode, i64 %value) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_sem_open (i64 %name, i64 %oflag, i64 %mode, i64 %value);
  ret i64 %r;
  ret i64 0
}

define internal i64 @sem_wait(i64 %sem) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_sem_wait (i64 %sem);
  ret i64 %r;
  ret i64 0
}

define internal i64 @sem_post(i64 %sem) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_sem_post (i64 %sem);
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
  %r = call i64@briv_mkdir (i64 %path, i64 %mode);
  ret i64 %r;
  ret i64 0
}

define internal i64 @rmdir(i64 %path) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_rmdir (i64 %path);
  ret i64 %r;
  ret i64 0
}

define internal i64 @unlink(i64 %path) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_unlink (i64 %path);
  ret i64 %r;
  ret i64 0
}

define internal i64 @rename(i64 %oldpath, i64 %newpath) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_rename (i64 %oldpath, i64 %newpath);
  ret i64 %r;
  ret i64 0
}

define internal i64 @symlink(i64 %target, i64 %linkpath) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_symlink (i64 %target, i64 %linkpath);
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
  %r = call i64@briv_link (i64 %oldpath, i64 %newpath);
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
  %r = call i64@briv_chdir (i64 %path);
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
  %r = call i64@briv_chmod (i64 %path, i64 %mode);
  ret i64 %r;
  ret i64 0
}

define internal i64 @chown(i64 %path, i64 %owner, i64 %group) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_chown (i64 %path, i64 %owner, i64 %group);
  ret i64 %r;
  ret i64 0
}

define internal i64 @umask(i64 %mask) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_umask (i64 %mask);
  ret i64 %r;
  ret i64 0
}

define internal i64 @access(i64 %path, i64 %mode) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_access (i64 %path, i64 %mode);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getpid() local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_getpid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getppid() local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_getppid ();
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
  %r = call i64@briv_getuid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_geteuid() local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_geteuid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getgid() local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_getgid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getegid() local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_getegid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_clock_gettime(i64 %clock_id, i64 %tp) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_clock_gettime (i64 %clock_id, i64 %tp);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_nanosleep(i64 %req, i64 %rem) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_nanosleep (i64 %req, i64 %rem);
  ret i64 %r;
  ret i64 0
}

define internal i64 @mmap(i64 %addr, i64 %length, i64 %prot, i64 %flags, i64 %fd, i64 %offset) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_mmap (i64 %addr, i64 %length, i64 %prot, i64 %flags, i64 %fd, i64 %offset);
  ret i64 %r;
  ret i64 0
}

define internal i64 @munmap(i64 %addr, i64 %length) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_munmap (i64 %addr, i64 %length);
  ret i64 %r;
  ret i64 0
}

define internal i64 @mprotect(i64 %addr, i64 %length, i64 %prot) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_mprotect (i64 %addr, i64 %length, i64 %prot);
  ret i64 %r;
  ret i64 0
}

define internal i64 @brk(i64 %addr) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_brk (i64 %addr);
  ret i64 %r;
  ret i64 0
}

define internal i64 @mlock(i64 %addr, i64 %length) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_mlock (i64 %addr, i64 %length);
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
  %r = call i64@briv_sched_yield ();
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
  %r = call i64@briv_pagesize ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_cpu_count() local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_cpu_count ();
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
  %r = call i64@briv_ring_push (i64 %handle, i64 %val);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_ring_pop(i64 %handle) local_unnamed_addr #0 {
  entry:
  %r = call i64@briv_ring_pop (i64 %handle);
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
  %r = call i64@briv_futex (i64 %uaddr, i64 %opcode, i64 %val, i64 %timeout, i64 %uaddr2, i64 %val3);
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
%t194 = fmul fast float %t14, %t14
%t198 = fmul fast float %t20, %t20
%t199 = fadd fast float %t194, %t198
%t203 = fmul fast float %t26, %t26
%t204 = fadd fast float %t199, %t203
  ; let dsq01 = %t204
%t210 = fmul fast float %t32, %t32
%t214 = fmul fast float %t38, %t38
%t215 = fadd fast float %t210, %t214
%t219 = fmul fast float %t44, %t44
%t220 = fadd fast float %t215, %t219
  ; let dsq02 = %t220
%t226 = fmul fast float %t50, %t50
%t230 = fmul fast float %t56, %t56
%t231 = fadd fast float %t226, %t230
%t235 = fmul fast float %t62, %t62
%t236 = fadd fast float %t231, %t235
  ; let dsq03 = %t236
%t242 = fmul fast float %t68, %t68
%t246 = fmul fast float %t74, %t74
%t247 = fadd fast float %t242, %t246
%t251 = fmul fast float %t80, %t80
%t252 = fadd fast float %t247, %t251
  ; let dsq04 = %t252
%t258 = fmul fast float %t86, %t86
%t262 = fmul fast float %t92, %t92
%t263 = fadd fast float %t258, %t262
%t267 = fmul fast float %t98, %t98
%t268 = fadd fast float %t263, %t267
  ; let dsq12 = %t268
%t274 = fmul fast float %t104, %t104
%t278 = fmul fast float %t110, %t110
%t279 = fadd fast float %t274, %t278
%t283 = fmul fast float %t116, %t116
%t284 = fadd fast float %t279, %t283
  ; let dsq13 = %t284
%t290 = fmul fast float %t122, %t122
%t294 = fmul fast float %t128, %t128
%t295 = fadd fast float %t290, %t294
%t299 = fmul fast float %t134, %t134
%t300 = fadd fast float %t295, %t299
  ; let dsq14 = %t300
%t306 = fmul fast float %t140, %t140
%t310 = fmul fast float %t146, %t146
%t311 = fadd fast float %t306, %t310
%t315 = fmul fast float %t152, %t152
%t316 = fadd fast float %t311, %t315
  ; let dsq23 = %t316
%t322 = fmul fast float %t158, %t158
%t326 = fmul fast float %t164, %t164
%t327 = fadd fast float %t322, %t326
%t331 = fmul fast float %t170, %t170
%t332 = fadd fast float %t327, %t331
  ; let dsq24 = %t332
%t338 = fmul fast float %t176, %t176
%t342 = fmul fast float %t182, %t182
%t343 = fadd fast float %t338, %t342
%t347 = fmul fast float %t188, %t188
%t348 = fadd fast float %t343, %t347
  ; let dsq34 = %t348
  %t349 = call float @llvm.sqrt.f32(float %t204)
  ; let dist01 = %t349
  %t351 = call float @llvm.sqrt.f32(float %t220)
  ; let dist02 = %t351
  %t353 = call float @llvm.sqrt.f32(float %t236)
  ; let dist03 = %t353
  %t355 = call float @llvm.sqrt.f32(float %t252)
  ; let dist04 = %t355
  %t357 = call float @llvm.sqrt.f32(float %t268)
  ; let dist12 = %t357
  %t359 = call float @llvm.sqrt.f32(float %t284)
  ; let dist13 = %t359
  %t361 = call float @llvm.sqrt.f32(float %t300)
  ; let dist14 = %t361
  %t363 = call float @llvm.sqrt.f32(float %t316)
  ; let dist23 = %t363
  %t365 = call float @llvm.sqrt.f32(float %t332)
  ; let dist24 = %t365
  %t367 = call float @llvm.sqrt.f32(float %t348)
  ; let dist34 = %t367
  %il371 = load float, float* @dt, align 4
%t375 = fmul fast float %t204, %t349
%t376 = fdiv fast float %il371, %t375
  ; let mag01 = %t376
  %il379 = load float, float* @dt, align 4
%t383 = fmul fast float %t220, %t351
%t384 = fdiv fast float %il379, %t383
  ; let mag02 = %t384
  %il387 = load float, float* @dt, align 4
%t391 = fmul fast float %t236, %t353
%t392 = fdiv fast float %il387, %t391
  ; let mag03 = %t392
  %il395 = load float, float* @dt, align 4
%t399 = fmul fast float %t252, %t355
%t400 = fdiv fast float %il395, %t399
  ; let mag04 = %t400
  %il403 = load float, float* @dt, align 4
%t407 = fmul fast float %t268, %t357
%t408 = fdiv fast float %il403, %t407
  ; let mag12 = %t408
  %il411 = load float, float* @dt, align 4
%t415 = fmul fast float %t284, %t359
%t416 = fdiv fast float %il411, %t415
  ; let mag13 = %t416
  %il419 = load float, float* @dt, align 4
%t423 = fmul fast float %t300, %t361
%t424 = fdiv fast float %il419, %t423
  ; let mag14 = %t424
  %il427 = load float, float* @dt, align 4
%t431 = fmul fast float %t316, %t363
%t432 = fdiv fast float %il427, %t431
  ; let mag23 = %t432
  %il435 = load float, float* @dt, align 4
%t439 = fmul fast float %t332, %t365
%t440 = fdiv fast float %il435, %t439
  ; let mag24 = %t440
  %il443 = load float, float* @dt, align 4
%t447 = fmul fast float %t348, %t367
%t448 = fdiv fast float %il443, %t447
  ; let mag34 = %t448
  %fdp454 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t453 = load float, ptr %fdp454, align 4
  %il459 = load float, float* @m1, align 4
%t460 = fmul fast float %t20, %il459
%t462 = fmul fast float %t460, %t376
%t463 = fsub fast float %t453, %t462
  %il468 = load float, float* @m2, align 4
%t469 = fmul fast float %t38, %il468
%t471 = fmul fast float %t469, %t384
%t472 = fsub fast float %t463, %t471
  %il477 = load float, float* @m3, align 4
%t478 = fmul fast float %t56, %il477
%t480 = fmul fast float %t478, %t392
%t481 = fsub fast float %t472, %t480
  %il486 = load float, float* @m4, align 4
%t487 = fmul fast float %t74, %il486
%t489 = fmul fast float %t487, %t400
%t490 = fsub fast float %t481, %t489
  ; let nvy0 = %t490
  %fdp496 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t495 = load float, ptr %fdp496, align 4
  %il501 = load float, float* @m1, align 4
%t502 = fmul fast float %t14, %il501
%t504 = fmul fast float %t502, %t376
%t505 = fsub fast float %t495, %t504
  %il510 = load float, float* @m2, align 4
%t511 = fmul fast float %t32, %il510
%t513 = fmul fast float %t511, %t384
%t514 = fsub fast float %t505, %t513
  %il519 = load float, float* @m3, align 4
%t520 = fmul fast float %t50, %il519
%t522 = fmul fast float %t520, %t392
%t523 = fsub fast float %t514, %t522
  %il528 = load float, float* @m4, align 4
%t529 = fmul fast float %t68, %il528
%t531 = fmul fast float %t529, %t400
%t532 = fsub fast float %t523, %t531
  ; let nvx0 = %t532
  %fdp538 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t537 = load float, ptr %fdp538, align 4
  %il543 = load float, float* @m1, align 4
%t544 = fmul fast float %t26, %il543
%t546 = fmul fast float %t544, %t376
%t547 = fsub fast float %t537, %t546
  %il552 = load float, float* @m2, align 4
%t553 = fmul fast float %t44, %il552
%t555 = fmul fast float %t553, %t384
%t556 = fsub fast float %t547, %t555
  %il561 = load float, float* @m3, align 4
%t562 = fmul fast float %t62, %il561
%t564 = fmul fast float %t562, %t392
%t565 = fsub fast float %t556, %t564
  %il570 = load float, float* @m4, align 4
%t571 = fmul fast float %t80, %il570
%t573 = fmul fast float %t571, %t400
%t574 = fsub fast float %t565, %t573
  ; let nvz0 = %t574
  %fdp580 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t579 = load float, ptr %fdp580, align 4
  %il585 = load float, float* @m0, align 4
%t586 = fmul fast float %t26, %il585
%t588 = fmul fast float %t586, %t376
%t589 = fadd fast float %t579, %t588
  %il594 = load float, float* @m2, align 4
%t595 = fmul fast float %t98, %il594
%t597 = fmul fast float %t595, %t408
%t598 = fsub fast float %t589, %t597
  %il603 = load float, float* @m3, align 4
%t604 = fmul fast float %t116, %il603
%t606 = fmul fast float %t604, %t416
%t607 = fsub fast float %t598, %t606
  %il612 = load float, float* @m4, align 4
%t613 = fmul fast float %t134, %il612
%t615 = fmul fast float %t613, %t424
%t616 = fsub fast float %t607, %t615
  ; let nvz1 = %t616
  %fdp622 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t621 = load float, ptr %fdp622, align 4
  %il627 = load float, float* @m0, align 4
%t628 = fmul fast float %t14, %il627
%t630 = fmul fast float %t628, %t376
%t631 = fadd fast float %t621, %t630
  %il636 = load float, float* @m2, align 4
%t637 = fmul fast float %t86, %il636
%t639 = fmul fast float %t637, %t408
%t640 = fsub fast float %t631, %t639
  %il645 = load float, float* @m3, align 4
%t646 = fmul fast float %t104, %il645
%t648 = fmul fast float %t646, %t416
%t649 = fsub fast float %t640, %t648
  %il654 = load float, float* @m4, align 4
%t655 = fmul fast float %t122, %il654
%t657 = fmul fast float %t655, %t424
%t658 = fsub fast float %t649, %t657
  ; let nvx1 = %t658
  %fdp664 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t663 = load float, ptr %fdp664, align 4
  %il669 = load float, float* @m0, align 4
%t670 = fmul fast float %t20, %il669
%t672 = fmul fast float %t670, %t376
%t673 = fadd fast float %t663, %t672
  %il678 = load float, float* @m2, align 4
%t679 = fmul fast float %t92, %il678
%t681 = fmul fast float %t679, %t408
%t682 = fsub fast float %t673, %t681
  %il687 = load float, float* @m3, align 4
%t688 = fmul fast float %t110, %il687
%t690 = fmul fast float %t688, %t416
%t691 = fsub fast float %t682, %t690
  %il696 = load float, float* @m4, align 4
%t697 = fmul fast float %t128, %il696
%t699 = fmul fast float %t697, %t424
%t700 = fsub fast float %t691, %t699
  ; let nvy1 = %t700
  %fdp706 = getelementptr inbounds %State, ptr %state, i32 0, i32 19
  %t705 = load float, ptr %fdp706, align 4
  %il711 = load float, float* @m0, align 4
%t712 = fmul fast float %t44, %il711
%t714 = fmul fast float %t712, %t384
%t715 = fadd fast float %t705, %t714
  %il720 = load float, float* @m1, align 4
%t721 = fmul fast float %t98, %il720
%t723 = fmul fast float %t721, %t408
%t724 = fadd fast float %t715, %t723
  %il729 = load float, float* @m3, align 4
%t730 = fmul fast float %t152, %il729
%t732 = fmul fast float %t730, %t432
%t733 = fsub fast float %t724, %t732
  %il738 = load float, float* @m4, align 4
%t739 = fmul fast float %t170, %il738
%t741 = fmul fast float %t739, %t440
%t742 = fsub fast float %t733, %t741
  ; let nvz2 = %t742
  %fdp748 = getelementptr inbounds %State, ptr %state, i32 0, i32 18
  %t747 = load float, ptr %fdp748, align 4
  %il753 = load float, float* @m0, align 4
%t754 = fmul fast float %t38, %il753
%t756 = fmul fast float %t754, %t384
%t757 = fadd fast float %t747, %t756
  %il762 = load float, float* @m1, align 4
%t763 = fmul fast float %t92, %il762
%t765 = fmul fast float %t763, %t408
%t766 = fadd fast float %t757, %t765
  %il771 = load float, float* @m3, align 4
%t772 = fmul fast float %t146, %il771
%t774 = fmul fast float %t772, %t432
%t775 = fsub fast float %t766, %t774
  %il780 = load float, float* @m4, align 4
%t781 = fmul fast float %t164, %il780
%t783 = fmul fast float %t781, %t440
%t784 = fsub fast float %t775, %t783
  ; let nvy2 = %t784
  %fdp790 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
  %t789 = load float, ptr %fdp790, align 4
  %il795 = load float, float* @m0, align 4
%t796 = fmul fast float %t32, %il795
%t798 = fmul fast float %t796, %t384
%t799 = fadd fast float %t789, %t798
  %il804 = load float, float* @m1, align 4
%t805 = fmul fast float %t86, %il804
%t807 = fmul fast float %t805, %t408
%t808 = fadd fast float %t799, %t807
  %il813 = load float, float* @m3, align 4
%t814 = fmul fast float %t140, %il813
%t816 = fmul fast float %t814, %t432
%t817 = fsub fast float %t808, %t816
  %il822 = load float, float* @m4, align 4
%t823 = fmul fast float %t158, %il822
%t825 = fmul fast float %t823, %t440
%t826 = fsub fast float %t817, %t825
  ; let nvx2 = %t826
  %fdp832 = getelementptr inbounds %State, ptr %state, i32 0, i32 30
  %t831 = load float, ptr %fdp832, align 4
  %il837 = load float, float* @m0, align 4
%t838 = fmul fast float %t74, %il837
%t840 = fmul fast float %t838, %t400
%t841 = fadd fast float %t831, %t840
  %il846 = load float, float* @m1, align 4
%t847 = fmul fast float %t128, %il846
%t849 = fmul fast float %t847, %t424
%t850 = fadd fast float %t841, %t849
  %il855 = load float, float* @m2, align 4
%t856 = fmul fast float %t164, %il855
%t858 = fmul fast float %t856, %t440
%t859 = fadd fast float %t850, %t858
  %il864 = load float, float* @m3, align 4
%t865 = fmul fast float %t182, %il864
%t867 = fmul fast float %t865, %t448
%t868 = fadd fast float %t859, %t867
  ; let nvy4 = %t868
  %fdp874 = getelementptr inbounds %State, ptr %state, i32 0, i32 24
  %t873 = load float, ptr %fdp874, align 4
  %il879 = load float, float* @m0, align 4
%t880 = fmul fast float %t56, %il879
%t882 = fmul fast float %t880, %t392
%t883 = fadd fast float %t873, %t882
  %il888 = load float, float* @m1, align 4
%t889 = fmul fast float %t110, %il888
%t891 = fmul fast float %t889, %t416
%t892 = fadd fast float %t883, %t891
  %il897 = load float, float* @m2, align 4
%t898 = fmul fast float %t146, %il897
%t900 = fmul fast float %t898, %t432
%t901 = fadd fast float %t892, %t900
  %il906 = load float, float* @m4, align 4
%t907 = fmul fast float %t176, %il906
%t909 = fmul fast float %t907, %t448
%t910 = fsub fast float %t901, %t909
  ; let nvy3 = %t910
  %fdp916 = getelementptr inbounds %State, ptr %state, i32 0, i32 23
  %t915 = load float, ptr %fdp916, align 4
  %il921 = load float, float* @m0, align 4
%t922 = fmul fast float %t50, %il921
%t924 = fmul fast float %t922, %t392
%t925 = fadd fast float %t915, %t924
  %il930 = load float, float* @m1, align 4
%t931 = fmul fast float %t104, %il930
%t933 = fmul fast float %t931, %t416
%t934 = fadd fast float %t925, %t933
  %il939 = load float, float* @m2, align 4
%t940 = fmul fast float %t140, %il939
%t942 = fmul fast float %t940, %t432
%t943 = fadd fast float %t934, %t942
  %il948 = load float, float* @m4, align 4
%t949 = fmul fast float %t176, %il948
%t951 = fmul fast float %t949, %t448
%t952 = fsub fast float %t943, %t951
  ; let nvx3 = %t952
  %fdp958 = getelementptr inbounds %State, ptr %state, i32 0, i32 29
  %t957 = load float, ptr %fdp958, align 4
  %il963 = load float, float* @m0, align 4
%t964 = fmul fast float %t68, %il963
%t966 = fmul fast float %t964, %t400
%t967 = fadd fast float %t957, %t966
  %il972 = load float, float* @m1, align 4
%t973 = fmul fast float %t122, %il972
%t975 = fmul fast float %t973, %t424
%t976 = fadd fast float %t967, %t975
  %il981 = load float, float* @m2, align 4
%t982 = fmul fast float %t158, %il981
%t984 = fmul fast float %t982, %t440
%t985 = fadd fast float %t976, %t984
  %il990 = load float, float* @m3, align 4
%t991 = fmul fast float %t176, %il990
%t993 = fmul fast float %t991, %t448
%t994 = fadd fast float %t985, %t993
  ; let nvx4 = %t994
  %fdp1000 = getelementptr inbounds %State, ptr %state, i32 0, i32 25
  %t999 = load float, ptr %fdp1000, align 4
  %il1005 = load float, float* @m0, align 4
%t1006 = fmul fast float %t62, %il1005
%t1008 = fmul fast float %t1006, %t392
%t1009 = fadd fast float %t999, %t1008
  %il1014 = load float, float* @m1, align 4
%t1015 = fmul fast float %t116, %il1014
%t1017 = fmul fast float %t1015, %t416
%t1018 = fadd fast float %t1009, %t1017
  %il1023 = load float, float* @m2, align 4
%t1024 = fmul fast float %t152, %il1023
%t1026 = fmul fast float %t1024, %t432
%t1027 = fadd fast float %t1018, %t1026
  %il1032 = load float, float* @m4, align 4
%t1033 = fmul fast float %t188, %il1032
%t1035 = fmul fast float %t1033, %t448
%t1036 = fsub fast float %t1027, %t1035
  ; let nvz3 = %t1036
  %fdp1042 = getelementptr inbounds %State, ptr %state, i32 0, i32 31
  %t1041 = load float, ptr %fdp1042, align 4
  %il1047 = load float, float* @m0, align 4
%t1048 = fmul fast float %t80, %il1047
%t1050 = fmul fast float %t1048, %t400
%t1051 = fadd fast float %t1041, %t1050
  %il1056 = load float, float* @m1, align 4
%t1057 = fmul fast float %t134, %il1056
%t1059 = fmul fast float %t1057, %t424
%t1060 = fadd fast float %t1051, %t1059
  %il1065 = load float, float* @m2, align 4
%t1066 = fmul fast float %t170, %il1065
%t1068 = fmul fast float %t1066, %t440
%t1069 = fadd fast float %t1060, %t1068
  %il1074 = load float, float* @m3, align 4
%t1075 = fmul fast float %t188, %il1074
%t1077 = fmul fast float %t1075, %t448
%t1078 = fadd fast float %t1069, %t1077
  ; let nvz4 = %t1078
  %fdp1081 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1080 = load float, ptr %fdp1081, align 4
  %il1084 = load float, float* @dt, align 4
%t1086 = fmul fast float %il1084, %t490
%t1087 = fadd fast float %t1080, %t1086
  %ap_1088 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store float %t1087, ptr %ap_1088, align 4, !tbaa !3
  %ap_1090 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store float %t490, ptr %ap_1090, align 4, !tbaa !3
  %fdp1093 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1092 = load float, ptr %fdp1093, align 4
  %il1096 = load float, float* @dt, align 4
%t1098 = fmul fast float %il1096, %t532
%t1099 = fadd fast float %t1092, %t1098
  %ap_1100 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store float %t1099, ptr %ap_1100, align 4, !tbaa !3
  %ap_1102 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store float %t532, ptr %ap_1102, align 4, !tbaa !3
  %fdp1105 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1104 = load float, ptr %fdp1105, align 4
  %il1108 = load float, float* @dt, align 4
%t1110 = fmul fast float %il1108, %t574
%t1111 = fadd fast float %t1104, %t1110
  %ap_1112 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store float %t1111, ptr %ap_1112, align 4, !tbaa !3
  %ap_1114 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store float %t574, ptr %ap_1114, align 4, !tbaa !3
  %fdp1117 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1116 = load float, ptr %fdp1117, align 4
  %il1120 = load float, float* @dt, align 4
%t1122 = fmul fast float %il1120, %t616
%t1123 = fadd fast float %t1116, %t1122
  %ap_1124 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store float %t1123, ptr %ap_1124, align 4, !tbaa !3
  %ap_1126 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  store float %t616, ptr %ap_1126, align 4, !tbaa !3
  %fdp1129 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1128 = load float, ptr %fdp1129, align 4
  %il1132 = load float, float* @dt, align 4
%t1134 = fmul fast float %il1132, %t658
%t1135 = fadd fast float %t1128, %t1134
  %ap_1136 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store float %t1135, ptr %ap_1136, align 4, !tbaa !3
  %ap_1138 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  store float %t658, ptr %ap_1138, align 4, !tbaa !3
  %ap_1140 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store float %t700, ptr %ap_1140, align 4, !tbaa !3
  %fdp1143 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1142 = load float, ptr %fdp1143, align 4
  %il1146 = load float, float* @dt, align 4
%t1148 = fmul fast float %il1146, %t700
%t1149 = fadd fast float %t1142, %t1148
  %ap_1150 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store float %t1149, ptr %ap_1150, align 4, !tbaa !3
  %fdp1153 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1152 = load float, ptr %fdp1153, align 4
  %il1156 = load float, float* @dt, align 4
%t1158 = fmul fast float %il1156, %t742
%t1159 = fadd fast float %t1152, %t1158
  %ap_1160 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  store float %t1159, ptr %ap_1160, align 4, !tbaa !3
  %ap_1162 = getelementptr inbounds %State, ptr %state, i32 0, i32 19
  store float %t742, ptr %ap_1162, align 4, !tbaa !3
  %ap_1164 = getelementptr inbounds %State, ptr %state, i32 0, i32 18
  store float %t784, ptr %ap_1164, align 4, !tbaa !3
  %fdp1167 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1166 = load float, ptr %fdp1167, align 4
  %il1170 = load float, float* @dt, align 4
%t1172 = fmul fast float %il1170, %t784
%t1173 = fadd fast float %t1166, %t1172
  %ap_1174 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  store float %t1173, ptr %ap_1174, align 4, !tbaa !3
  %ap_1176 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
  store float %t826, ptr %ap_1176, align 4, !tbaa !3
  %fdp1179 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1178 = load float, ptr %fdp1179, align 4
  %il1182 = load float, float* @dt, align 4
%t1184 = fmul fast float %il1182, %t826
%t1185 = fadd fast float %t1178, %t1184
  %ap_1186 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  store float %t1185, ptr %ap_1186, align 4, !tbaa !3
  %ap_1188 = getelementptr inbounds %State, ptr %state, i32 0, i32 30
  store float %t868, ptr %ap_1188, align 4, !tbaa !3
  %fdp1191 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1190 = load float, ptr %fdp1191, align 4
  %il1194 = load float, float* @dt, align 4
%t1196 = fmul fast float %il1194, %t868
%t1197 = fadd fast float %t1190, %t1196
  %ap_1198 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  store float %t1197, ptr %ap_1198, align 4, !tbaa !3
  %ap_1200 = getelementptr inbounds %State, ptr %state, i32 0, i32 24
  store float %t910, ptr %ap_1200, align 4, !tbaa !3
  %fdp1203 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1202 = load float, ptr %fdp1203, align 4
  %il1206 = load float, float* @dt, align 4
%t1208 = fmul fast float %il1206, %t910
%t1209 = fadd fast float %t1202, %t1208
  %ap_1210 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  store float %t1209, ptr %ap_1210, align 4, !tbaa !3
  %fdp1213 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1212 = load float, ptr %fdp1213, align 4
  %il1216 = load float, float* @dt, align 4
%t1218 = fmul fast float %il1216, %t952
%t1219 = fadd fast float %t1212, %t1218
  %ap_1220 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  store float %t1219, ptr %ap_1220, align 4, !tbaa !3
  %ap_1222 = getelementptr inbounds %State, ptr %state, i32 0, i32 23
  store float %t952, ptr %ap_1222, align 4, !tbaa !3
  %ap_1224 = getelementptr inbounds %State, ptr %state, i32 0, i32 29
  store float %t994, ptr %ap_1224, align 4, !tbaa !3
  %fdp1227 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1226 = load float, ptr %fdp1227, align 4
  %il1230 = load float, float* @dt, align 4
%t1232 = fmul fast float %il1230, %t994
%t1233 = fadd fast float %t1226, %t1232
  %ap_1234 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  store float %t1233, ptr %ap_1234, align 4, !tbaa !3
  %fdp1237 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1236 = load float, ptr %fdp1237, align 4
  %il1240 = load float, float* @dt, align 4
%t1242 = fmul fast float %il1240, %t1036
%t1243 = fadd fast float %t1236, %t1242
  %ap_1244 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  store float %t1243, ptr %ap_1244, align 4, !tbaa !3
  %ap_1246 = getelementptr inbounds %State, ptr %state, i32 0, i32 25
  store float %t1036, ptr %ap_1246, align 4, !tbaa !3
  %ap_1248 = getelementptr inbounds %State, ptr %state, i32 0, i32 31
  store float %t1078, ptr %ap_1248, align 4, !tbaa !3
  %fdp1251 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1250 = load float, ptr %fdp1251, align 4
  %il1254 = load float, float* @dt, align 4
%t1256 = fmul fast float %il1254, %t1078
%t1257 = fadd fast float %t1250, %t1256
  %ap_1258 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  store float %t1257, ptr %ap_1258, align 4, !tbaa !3
%ff1263 = bitcast i32 1056964608 to float
  %il1265 = load float, float* @m0, align 4
%t1266 = fmul fast float %ff1263, %il1265
  %fdp1271 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t1270 = load float, ptr %fdp1271, align 4
  %fdp1273 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t1272 = load float, ptr %fdp1273, align 4
%t1274 = fmul fast float %t1270, %t1272
  %fdp1277 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t1276 = load float, ptr %fdp1277, align 4
  %fdp1279 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t1278 = load float, ptr %fdp1279, align 4
%t1280 = fmul fast float %t1276, %t1278
%t1281 = fadd fast float %t1274, %t1280
  %fdp1284 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t1283 = load float, ptr %fdp1284, align 4
  %fdp1286 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t1285 = load float, ptr %fdp1286, align 4
%t1287 = fmul fast float %t1283, %t1285
%t1288 = fadd fast float %t1281, %t1287
%t1289 = fmul fast float %t1266, %t1288
  ; let ek0 = %t1289
%ff1294 = bitcast i32 1056964608 to float
  %il1296 = load float, float* @m1, align 4
%t1297 = fmul fast float %ff1294, %il1296
  %fdp1302 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t1301 = load float, ptr %fdp1302, align 4
  %fdp1304 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t1303 = load float, ptr %fdp1304, align 4
%t1305 = fmul fast float %t1301, %t1303
  %fdp1308 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t1307 = load float, ptr %fdp1308, align 4
  %fdp1310 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t1309 = load float, ptr %fdp1310, align 4
%t1311 = fmul fast float %t1307, %t1309
%t1312 = fadd fast float %t1305, %t1311
  %fdp1315 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t1314 = load float, ptr %fdp1315, align 4
  %fdp1317 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t1316 = load float, ptr %fdp1317, align 4
%t1318 = fmul fast float %t1314, %t1316
%t1319 = fadd fast float %t1312, %t1318
%t1320 = fmul fast float %t1297, %t1319
  ; let ek1 = %t1320
  %fdp1327 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1326 = load float, ptr %fdp1327, align 4
  %fdp1329 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1328 = load float, ptr %fdp1329, align 4
%t1330 = fsub fast float %t1326, %t1328
  %fdp1333 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1332 = load float, ptr %fdp1333, align 4
  %fdp1335 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1334 = load float, ptr %fdp1335, align 4
%t1336 = fsub fast float %t1332, %t1334
%t1337 = fmul fast float %t1330, %t1336
  %fdp1341 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1340 = load float, ptr %fdp1341, align 4
  %fdp1343 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1342 = load float, ptr %fdp1343, align 4
%t1344 = fsub fast float %t1340, %t1342
  %fdp1347 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1346 = load float, ptr %fdp1347, align 4
  %fdp1349 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1348 = load float, ptr %fdp1349, align 4
%t1350 = fsub fast float %t1346, %t1348
%t1351 = fmul fast float %t1344, %t1350
%t1352 = fadd fast float %t1337, %t1351
  %fdp1356 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1355 = load float, ptr %fdp1356, align 4
  %fdp1358 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1357 = load float, ptr %fdp1358, align 4
%t1359 = fsub fast float %t1355, %t1357
  %fdp1362 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1361 = load float, ptr %fdp1362, align 4
  %fdp1364 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1363 = load float, ptr %fdp1364, align 4
%t1365 = fsub fast float %t1361, %t1363
%t1366 = fmul fast float %t1359, %t1365
%t1367 = fadd fast float %t1352, %t1366
  %t1321 = call float @llvm.sqrt.f32(float %t1367)
  ; let edist01 = %t1321
%ff1372 = bitcast i32 1056964608 to float
  %il1374 = load float, float* @m2, align 4
%t1375 = fmul fast float %ff1372, %il1374
  %fdp1380 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
  %t1379 = load float, ptr %fdp1380, align 4
  %fdp1382 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
  %t1381 = load float, ptr %fdp1382, align 4
%t1383 = fmul fast float %t1379, %t1381
  %fdp1386 = getelementptr inbounds %State, ptr %state, i32 0, i32 18
  %t1385 = load float, ptr %fdp1386, align 4
  %fdp1388 = getelementptr inbounds %State, ptr %state, i32 0, i32 18
  %t1387 = load float, ptr %fdp1388, align 4
%t1389 = fmul fast float %t1385, %t1387
%t1390 = fadd fast float %t1383, %t1389
  %fdp1393 = getelementptr inbounds %State, ptr %state, i32 0, i32 19
  %t1392 = load float, ptr %fdp1393, align 4
  %fdp1395 = getelementptr inbounds %State, ptr %state, i32 0, i32 19
  %t1394 = load float, ptr %fdp1395, align 4
%t1396 = fmul fast float %t1392, %t1394
%t1397 = fadd fast float %t1390, %t1396
%t1398 = fmul fast float %t1375, %t1397
  ; let ek2 = %t1398
  %fdp1405 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1404 = load float, ptr %fdp1405, align 4
  %fdp1407 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1406 = load float, ptr %fdp1407, align 4
%t1408 = fsub fast float %t1404, %t1406
  %fdp1411 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1410 = load float, ptr %fdp1411, align 4
  %fdp1413 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1412 = load float, ptr %fdp1413, align 4
%t1414 = fsub fast float %t1410, %t1412
%t1415 = fmul fast float %t1408, %t1414
  %fdp1419 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1418 = load float, ptr %fdp1419, align 4
  %fdp1421 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1420 = load float, ptr %fdp1421, align 4
%t1422 = fsub fast float %t1418, %t1420
  %fdp1425 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1424 = load float, ptr %fdp1425, align 4
  %fdp1427 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1426 = load float, ptr %fdp1427, align 4
%t1428 = fsub fast float %t1424, %t1426
%t1429 = fmul fast float %t1422, %t1428
%t1430 = fadd fast float %t1415, %t1429
  %fdp1434 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1433 = load float, ptr %fdp1434, align 4
  %fdp1436 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1435 = load float, ptr %fdp1436, align 4
%t1437 = fsub fast float %t1433, %t1435
  %fdp1440 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1439 = load float, ptr %fdp1440, align 4
  %fdp1442 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1441 = load float, ptr %fdp1442, align 4
%t1443 = fsub fast float %t1439, %t1441
%t1444 = fmul fast float %t1437, %t1443
%t1445 = fadd fast float %t1430, %t1444
  %t1399 = call float @llvm.sqrt.f32(float %t1445)
  ; let edist12 = %t1399
  %fdp1452 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1451 = load float, ptr %fdp1452, align 4
  %fdp1454 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1453 = load float, ptr %fdp1454, align 4
%t1455 = fsub fast float %t1451, %t1453
  %fdp1458 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1457 = load float, ptr %fdp1458, align 4
  %fdp1460 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1459 = load float, ptr %fdp1460, align 4
%t1461 = fsub fast float %t1457, %t1459
%t1462 = fmul fast float %t1455, %t1461
  %fdp1466 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1465 = load float, ptr %fdp1466, align 4
  %fdp1468 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1467 = load float, ptr %fdp1468, align 4
%t1469 = fsub fast float %t1465, %t1467
  %fdp1472 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1471 = load float, ptr %fdp1472, align 4
  %fdp1474 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1473 = load float, ptr %fdp1474, align 4
%t1475 = fsub fast float %t1471, %t1473
%t1476 = fmul fast float %t1469, %t1475
%t1477 = fadd fast float %t1462, %t1476
  %fdp1481 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1480 = load float, ptr %fdp1481, align 4
  %fdp1483 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1482 = load float, ptr %fdp1483, align 4
%t1484 = fsub fast float %t1480, %t1482
  %fdp1487 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1486 = load float, ptr %fdp1487, align 4
  %fdp1489 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1488 = load float, ptr %fdp1489, align 4
%t1490 = fsub fast float %t1486, %t1488
%t1491 = fmul fast float %t1484, %t1490
%t1492 = fadd fast float %t1477, %t1491
  %t1446 = call float @llvm.sqrt.f32(float %t1492)
  ; let edist02 = %t1446
  %fdp1499 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1498 = load float, ptr %fdp1499, align 4
  %fdp1501 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1500 = load float, ptr %fdp1501, align 4
%t1502 = fsub fast float %t1498, %t1500
  %fdp1505 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1504 = load float, ptr %fdp1505, align 4
  %fdp1507 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1506 = load float, ptr %fdp1507, align 4
%t1508 = fsub fast float %t1504, %t1506
%t1509 = fmul fast float %t1502, %t1508
  %fdp1513 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1512 = load float, ptr %fdp1513, align 4
  %fdp1515 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1514 = load float, ptr %fdp1515, align 4
%t1516 = fsub fast float %t1512, %t1514
  %fdp1519 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1518 = load float, ptr %fdp1519, align 4
  %fdp1521 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1520 = load float, ptr %fdp1521, align 4
%t1522 = fsub fast float %t1518, %t1520
%t1523 = fmul fast float %t1516, %t1522
%t1524 = fadd fast float %t1509, %t1523
  %fdp1528 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1527 = load float, ptr %fdp1528, align 4
  %fdp1530 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1529 = load float, ptr %fdp1530, align 4
%t1531 = fsub fast float %t1527, %t1529
  %fdp1534 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1533 = load float, ptr %fdp1534, align 4
  %fdp1536 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1535 = load float, ptr %fdp1536, align 4
%t1537 = fsub fast float %t1533, %t1535
%t1538 = fmul fast float %t1531, %t1537
%t1539 = fadd fast float %t1524, %t1538
  %t1493 = call float @llvm.sqrt.f32(float %t1539)
  ; let edist13 = %t1493
  %fdp1546 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1545 = load float, ptr %fdp1546, align 4
  %fdp1548 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1547 = load float, ptr %fdp1548, align 4
%t1549 = fsub fast float %t1545, %t1547
  %fdp1552 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1551 = load float, ptr %fdp1552, align 4
  %fdp1554 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1553 = load float, ptr %fdp1554, align 4
%t1555 = fsub fast float %t1551, %t1553
%t1556 = fmul fast float %t1549, %t1555
  %fdp1560 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1559 = load float, ptr %fdp1560, align 4
  %fdp1562 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1561 = load float, ptr %fdp1562, align 4
%t1563 = fsub fast float %t1559, %t1561
  %fdp1566 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1565 = load float, ptr %fdp1566, align 4
  %fdp1568 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1567 = load float, ptr %fdp1568, align 4
%t1569 = fsub fast float %t1565, %t1567
%t1570 = fmul fast float %t1563, %t1569
%t1571 = fadd fast float %t1556, %t1570
  %fdp1575 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1574 = load float, ptr %fdp1575, align 4
  %fdp1577 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1576 = load float, ptr %fdp1577, align 4
%t1578 = fsub fast float %t1574, %t1576
  %fdp1581 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1580 = load float, ptr %fdp1581, align 4
  %fdp1583 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1582 = load float, ptr %fdp1583, align 4
%t1584 = fsub fast float %t1580, %t1582
%t1585 = fmul fast float %t1578, %t1584
%t1586 = fadd fast float %t1571, %t1585
  %t1540 = call float @llvm.sqrt.f32(float %t1586)
  ; let edist03 = %t1540
  %fdp1593 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1592 = load float, ptr %fdp1593, align 4
  %fdp1595 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1594 = load float, ptr %fdp1595, align 4
%t1596 = fsub fast float %t1592, %t1594
  %fdp1599 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1598 = load float, ptr %fdp1599, align 4
  %fdp1601 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1600 = load float, ptr %fdp1601, align 4
%t1602 = fsub fast float %t1598, %t1600
%t1603 = fmul fast float %t1596, %t1602
  %fdp1607 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1606 = load float, ptr %fdp1607, align 4
  %fdp1609 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1608 = load float, ptr %fdp1609, align 4
%t1610 = fsub fast float %t1606, %t1608
  %fdp1613 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1612 = load float, ptr %fdp1613, align 4
  %fdp1615 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1614 = load float, ptr %fdp1615, align 4
%t1616 = fsub fast float %t1612, %t1614
%t1617 = fmul fast float %t1610, %t1616
%t1618 = fadd fast float %t1603, %t1617
  %fdp1622 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1621 = load float, ptr %fdp1622, align 4
  %fdp1624 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1623 = load float, ptr %fdp1624, align 4
%t1625 = fsub fast float %t1621, %t1623
  %fdp1628 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1627 = load float, ptr %fdp1628, align 4
  %fdp1630 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1629 = load float, ptr %fdp1630, align 4
%t1631 = fsub fast float %t1627, %t1629
%t1632 = fmul fast float %t1625, %t1631
%t1633 = fadd fast float %t1618, %t1632
  %t1587 = call float @llvm.sqrt.f32(float %t1633)
  ; let edist23 = %t1587
%ff1638 = bitcast i32 1056964608 to float
  %il1640 = load float, float* @m3, align 4
%t1641 = fmul fast float %ff1638, %il1640
  %fdp1646 = getelementptr inbounds %State, ptr %state, i32 0, i32 23
  %t1645 = load float, ptr %fdp1646, align 4
  %fdp1648 = getelementptr inbounds %State, ptr %state, i32 0, i32 23
  %t1647 = load float, ptr %fdp1648, align 4
%t1649 = fmul fast float %t1645, %t1647
  %fdp1652 = getelementptr inbounds %State, ptr %state, i32 0, i32 24
  %t1651 = load float, ptr %fdp1652, align 4
  %fdp1654 = getelementptr inbounds %State, ptr %state, i32 0, i32 24
  %t1653 = load float, ptr %fdp1654, align 4
%t1655 = fmul fast float %t1651, %t1653
%t1656 = fadd fast float %t1649, %t1655
  %fdp1659 = getelementptr inbounds %State, ptr %state, i32 0, i32 25
  %t1658 = load float, ptr %fdp1659, align 4
  %fdp1661 = getelementptr inbounds %State, ptr %state, i32 0, i32 25
  %t1660 = load float, ptr %fdp1661, align 4
%t1662 = fmul fast float %t1658, %t1660
%t1663 = fadd fast float %t1656, %t1662
%t1664 = fmul fast float %t1641, %t1663
  ; let ek3 = %t1664
%ff1669 = bitcast i32 1056964608 to float
  %il1671 = load float, float* @m4, align 4
%t1672 = fmul fast float %ff1669, %il1671
  %fdp1677 = getelementptr inbounds %State, ptr %state, i32 0, i32 29
  %t1676 = load float, ptr %fdp1677, align 4
  %fdp1679 = getelementptr inbounds %State, ptr %state, i32 0, i32 29
  %t1678 = load float, ptr %fdp1679, align 4
%t1680 = fmul fast float %t1676, %t1678
  %fdp1683 = getelementptr inbounds %State, ptr %state, i32 0, i32 30
  %t1682 = load float, ptr %fdp1683, align 4
  %fdp1685 = getelementptr inbounds %State, ptr %state, i32 0, i32 30
  %t1684 = load float, ptr %fdp1685, align 4
%t1686 = fmul fast float %t1682, %t1684
%t1687 = fadd fast float %t1680, %t1686
  %fdp1690 = getelementptr inbounds %State, ptr %state, i32 0, i32 31
  %t1689 = load float, ptr %fdp1690, align 4
  %fdp1692 = getelementptr inbounds %State, ptr %state, i32 0, i32 31
  %t1691 = load float, ptr %fdp1692, align 4
%t1693 = fmul fast float %t1689, %t1691
%t1694 = fadd fast float %t1687, %t1693
%t1695 = fmul fast float %t1672, %t1694
  ; let ek4 = %t1695
  %fdp1702 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1701 = load float, ptr %fdp1702, align 4
  %fdp1704 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1703 = load float, ptr %fdp1704, align 4
%t1705 = fsub fast float %t1701, %t1703
  %fdp1708 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1707 = load float, ptr %fdp1708, align 4
  %fdp1710 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1709 = load float, ptr %fdp1710, align 4
%t1711 = fsub fast float %t1707, %t1709
%t1712 = fmul fast float %t1705, %t1711
  %fdp1716 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1715 = load float, ptr %fdp1716, align 4
  %fdp1718 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1717 = load float, ptr %fdp1718, align 4
%t1719 = fsub fast float %t1715, %t1717
  %fdp1722 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1721 = load float, ptr %fdp1722, align 4
  %fdp1724 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1723 = load float, ptr %fdp1724, align 4
%t1725 = fsub fast float %t1721, %t1723
%t1726 = fmul fast float %t1719, %t1725
%t1727 = fadd fast float %t1712, %t1726
  %fdp1731 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1730 = load float, ptr %fdp1731, align 4
  %fdp1733 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1732 = load float, ptr %fdp1733, align 4
%t1734 = fsub fast float %t1730, %t1732
  %fdp1737 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1736 = load float, ptr %fdp1737, align 4
  %fdp1739 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1738 = load float, ptr %fdp1739, align 4
%t1740 = fsub fast float %t1736, %t1738
%t1741 = fmul fast float %t1734, %t1740
%t1742 = fadd fast float %t1727, %t1741
  %t1696 = call float @llvm.sqrt.f32(float %t1742)
  ; let edist04 = %t1696
  %fdp1749 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1748 = load float, ptr %fdp1749, align 4
  %fdp1751 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1750 = load float, ptr %fdp1751, align 4
%t1752 = fsub fast float %t1748, %t1750
  %fdp1755 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1754 = load float, ptr %fdp1755, align 4
  %fdp1757 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1756 = load float, ptr %fdp1757, align 4
%t1758 = fsub fast float %t1754, %t1756
%t1759 = fmul fast float %t1752, %t1758
  %fdp1763 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1762 = load float, ptr %fdp1763, align 4
  %fdp1765 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1764 = load float, ptr %fdp1765, align 4
%t1766 = fsub fast float %t1762, %t1764
  %fdp1769 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1768 = load float, ptr %fdp1769, align 4
  %fdp1771 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1770 = load float, ptr %fdp1771, align 4
%t1772 = fsub fast float %t1768, %t1770
%t1773 = fmul fast float %t1766, %t1772
%t1774 = fadd fast float %t1759, %t1773
  %fdp1778 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1777 = load float, ptr %fdp1778, align 4
  %fdp1780 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1779 = load float, ptr %fdp1780, align 4
%t1781 = fsub fast float %t1777, %t1779
  %fdp1784 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1783 = load float, ptr %fdp1784, align 4
  %fdp1786 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1785 = load float, ptr %fdp1786, align 4
%t1787 = fsub fast float %t1783, %t1785
%t1788 = fmul fast float %t1781, %t1787
%t1789 = fadd fast float %t1774, %t1788
  %t1743 = call float @llvm.sqrt.f32(float %t1789)
  ; let edist24 = %t1743
  %fdp1796 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1795 = load float, ptr %fdp1796, align 4
  %fdp1798 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1797 = load float, ptr %fdp1798, align 4
%t1799 = fsub fast float %t1795, %t1797
  %fdp1802 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1801 = load float, ptr %fdp1802, align 4
  %fdp1804 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1803 = load float, ptr %fdp1804, align 4
%t1805 = fsub fast float %t1801, %t1803
%t1806 = fmul fast float %t1799, %t1805
  %fdp1810 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1809 = load float, ptr %fdp1810, align 4
  %fdp1812 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1811 = load float, ptr %fdp1812, align 4
%t1813 = fsub fast float %t1809, %t1811
  %fdp1816 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1815 = load float, ptr %fdp1816, align 4
  %fdp1818 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1817 = load float, ptr %fdp1818, align 4
%t1819 = fsub fast float %t1815, %t1817
%t1820 = fmul fast float %t1813, %t1819
%t1821 = fadd fast float %t1806, %t1820
  %fdp1825 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1824 = load float, ptr %fdp1825, align 4
  %fdp1827 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1826 = load float, ptr %fdp1827, align 4
%t1828 = fsub fast float %t1824, %t1826
  %fdp1831 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1830 = load float, ptr %fdp1831, align 4
  %fdp1833 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1832 = load float, ptr %fdp1833, align 4
%t1834 = fsub fast float %t1830, %t1832
%t1835 = fmul fast float %t1828, %t1834
%t1836 = fadd fast float %t1821, %t1835
  %t1790 = call float @llvm.sqrt.f32(float %t1836)
  ; let edist14 = %t1790
  %fdp1843 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1842 = load float, ptr %fdp1843, align 4
  %fdp1845 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1844 = load float, ptr %fdp1845, align 4
%t1846 = fsub fast float %t1842, %t1844
  %fdp1849 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1848 = load float, ptr %fdp1849, align 4
  %fdp1851 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1850 = load float, ptr %fdp1851, align 4
%t1852 = fsub fast float %t1848, %t1850
%t1853 = fmul fast float %t1846, %t1852
  %fdp1857 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1856 = load float, ptr %fdp1857, align 4
  %fdp1859 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1858 = load float, ptr %fdp1859, align 4
%t1860 = fsub fast float %t1856, %t1858
  %fdp1863 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1862 = load float, ptr %fdp1863, align 4
  %fdp1865 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1864 = load float, ptr %fdp1865, align 4
%t1866 = fsub fast float %t1862, %t1864
%t1867 = fmul fast float %t1860, %t1866
%t1868 = fadd fast float %t1853, %t1867
  %fdp1872 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1871 = load float, ptr %fdp1872, align 4
  %fdp1874 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1873 = load float, ptr %fdp1874, align 4
%t1875 = fsub fast float %t1871, %t1873
  %fdp1878 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1877 = load float, ptr %fdp1878, align 4
  %fdp1880 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1879 = load float, ptr %fdp1880, align 4
%t1881 = fsub fast float %t1877, %t1879
%t1882 = fmul fast float %t1875, %t1881
%t1883 = fadd fast float %t1868, %t1882
  %t1837 = call float @llvm.sqrt.f32(float %t1883)
  ; let edist34 = %t1837
  %il1887 = load float, float* @m0, align 4
  %il1889 = load float, float* @m1, align 4
%t1890 = fmul fast float %il1887, %il1889
%t1892 = fdiv fast float %t1890, %t1321
  ; let e01 = %t1892
  %il1896 = load float, float* @m1, align 4
  %il1898 = load float, float* @m2, align 4
%t1899 = fmul fast float %il1896, %il1898
%t1901 = fdiv fast float %t1899, %t1399
  ; let e12 = %t1901
  %il1905 = load float, float* @m0, align 4
  %il1907 = load float, float* @m2, align 4
%t1908 = fmul fast float %il1905, %il1907
%t1910 = fdiv fast float %t1908, %t1446
  ; let e02 = %t1910
  %il1914 = load float, float* @m1, align 4
  %il1916 = load float, float* @m3, align 4
%t1917 = fmul fast float %il1914, %il1916
%t1919 = fdiv fast float %t1917, %t1493
  ; let e13 = %t1919
  %il1923 = load float, float* @m0, align 4
  %il1925 = load float, float* @m3, align 4
%t1926 = fmul fast float %il1923, %il1925
%t1928 = fdiv fast float %t1926, %t1540
  ; let e03 = %t1928
  %il1932 = load float, float* @m2, align 4
  %il1934 = load float, float* @m3, align 4
%t1935 = fmul fast float %il1932, %il1934
%t1937 = fdiv fast float %t1935, %t1587
  ; let e23 = %t1937
  %il1941 = load float, float* @m0, align 4
  %il1943 = load float, float* @m4, align 4
%t1944 = fmul fast float %il1941, %il1943
%t1946 = fdiv fast float %t1944, %t1696
  ; let e04 = %t1946
  %il1950 = load float, float* @m2, align 4
  %il1952 = load float, float* @m4, align 4
%t1953 = fmul fast float %il1950, %il1952
%t1955 = fdiv fast float %t1953, %t1743
  ; let e24 = %t1955
  %il1959 = load float, float* @m1, align 4
  %il1961 = load float, float* @m4, align 4
%t1962 = fmul fast float %il1959, %il1961
%t1964 = fdiv fast float %t1962, %t1790
  ; let e14 = %t1964
  %il1968 = load float, float* @m3, align 4
  %il1970 = load float, float* @m4, align 4
%t1971 = fmul fast float %il1968, %il1970
%t1973 = fdiv fast float %t1971, %t1837
  ; let e34 = %t1973
%t1987 = fadd fast float %t1892, %t1910
%t1989 = fadd fast float %t1987, %t1928
%t1991 = fadd fast float %t1989, %t1946
%t1993 = fadd fast float %t1991, %t1901
%t1995 = fadd fast float %t1993, %t1919
%t1997 = fadd fast float %t1995, %t1964
%t1999 = fadd fast float %t1997, %t1937
%t2001 = fadd fast float %t1999, %t1955
%t2003 = fadd fast float %t2001, %t1973
  %t2004 = fneg float %t2003
  ; let ep = %t2004
%t2012 = fadd fast float %t2004, %t1289
%t2014 = fadd fast float %t2012, %t1320
%t2016 = fadd fast float %t2014, %t1398
%t2018 = fadd fast float %t2016, %t1664
%t2020 = fadd fast float %t2018, %t1695
  ; let energy = %t2020
  %fdp2024 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t2023 = load i64, i64* %fdp2024, align 8
%t2026 = add i64 0, 5000000
%t2027 = srem i64 %t2023, %t2026
%t2029 = add i64 0, 0
%c2030 = icmp eq i64 %t2027, %t2029
  br i1 %c2030, label %g2031_t, label %g2031_e
  g2031_t:
    %pfd2034 = fpext float %t2020 to double
    %pso2035 = load volatile ptr, ptr @stdout
    %pff2036 = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0
    %ppf2037 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso2035, ptr %pff2036, double %pfd2034)
    %t2032 = zext i32 %ppf2037 to i64
    ; let __printed = %t2032
    br label %g2031_tx
  g2031_tx:
    br label %g2031_e
  g2031_e:
  %fdp2040 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t2039 = load i64, i64* %fdp2040, align 8
%t2042 = add i64 0, 1
%t2043 = add nsw i64 %t2039, %t2042
  %ap_2044 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 %t2043, ptr %ap_2044, align 8, !tbaa !1
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
  store i64 0, ptr %ip_32, align 8
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
  store i64 0, ptr %ip_472, align 8
  %arptr3 = alloca i8*, align 8
  %arend3 = alloca i8*, align 8
  %arbase3 = alloca i8*, align 8
  %arinit3 = call ptr @malloc(i64 65536)
  store i8* %arinit3, ptr %arptr3, align 8
  store i8* %arinit3, ptr %arbase3, align 8
  %arieu3 = getelementptr i8, ptr %arinit3, i64 65536
  store i8* %arieu3, ptr %arend3, align 8
  %gt_473 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %cnt_bound_220 = load i64, ptr %gt_473, align 8
  br label %pre_phi
pre_phi:
  %init_cnt_474 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %init_bound_475 = load i64, ptr %init_cnt_474, align 8, !tbaa !1
  %init_cnt_476 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %init_bx0_477 = load float, ptr %init_cnt_476, align 4, !tbaa !3
  %init_cnt_478 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
  %init_bx1_479 = load float, ptr %init_cnt_478, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_480 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 14
  %init_bx2_481 = load float, ptr %init_cnt_480, align 4, !tbaa !3
  %init_cnt_482 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 5
  %init_bx3_483 = load float, ptr %init_cnt_482, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_484 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 11
  %init_bx4_485 = load float, ptr %init_cnt_484, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_486 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %init_by0_487 = load float, ptr %init_cnt_486, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_488 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
  %init_by1_489 = load float, ptr %init_cnt_488, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_490 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 0
  %init_by2_491 = load float, ptr %init_cnt_490, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_492 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 6
  %init_by3_493 = load float, ptr %init_cnt_492, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_494 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 12
  %init_by4_495 = load float, ptr %init_cnt_494, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_496 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %init_bz0_497 = load float, ptr %init_cnt_496, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_498 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
  %init_bz1_499 = load float, ptr %init_cnt_498, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_500 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 1
  %init_bz2_501 = load float, ptr %init_cnt_500, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_502 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 7
  %init_bz3_503 = load float, ptr %init_cnt_502, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_504 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 13
  %init_bz4_505 = load float, ptr %init_cnt_504, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_506 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 2
  %init_cycle_count_507 = load i64, ptr %init_cnt_506, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_508 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %init_vx0_509 = load float, ptr %init_cnt_508, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_510 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
  %init_vx1_511 = load float, ptr %init_cnt_510, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_512 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 2
  %init_vx2_513 = load float, ptr %init_cnt_512, align 4, !tbaa !3
  %init_cnt_514 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 8
  %init_vx3_515 = load float, ptr %init_cnt_514, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_516 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 14
  %init_vx4_517 = load float, ptr %init_cnt_516, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_518 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %init_vy0_519 = load float, ptr %init_cnt_518, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_520 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
  %init_vy1_521 = load float, ptr %init_cnt_520, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_522 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 3
  %init_vy2_523 = load float, ptr %init_cnt_522, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_524 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 9
  %init_vy3_525 = load float, ptr %init_cnt_524, align 4, !tbaa !3
  %init_cnt_526 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 0
  %init_vy4_527 = load float, ptr %init_cnt_526, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_528 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
  %init_vz0_529 = load float, ptr %init_cnt_528, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_530 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
  %init_vz1_531 = load float, ptr %init_cnt_530, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_532 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 4
  %init_vz2_533 = load float, ptr %init_cnt_532, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_534 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 10
  %init_vz3_535 = load float, ptr %init_cnt_534, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_536 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 1
  %init_vz4_537 = load float, ptr %init_cnt_536, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_538 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
  %init_count_539 = load i64, ptr %init_cnt_538, align 8
  %iv541_phi_by_v4541 = insertelement <4 x float> undef, float %init_by0_487, i32 0
  %iv542_phi_by_v4542 = insertelement <4 x float> %iv541_phi_by_v4541, float %init_by1_489, i32 1
  %iv543_phi_by_v4543 = insertelement <4 x float> %iv542_phi_by_v4542, float %init_by2_491, i32 2
  %iv544_phi_by_v4544 = insertelement <4 x float> %iv543_phi_by_v4543, float %init_by3_493, i32 3
  %iv545_phi_vy_v4545 = insertelement <4 x float> undef, float %init_vy0_519, i32 0
  %iv546_phi_vy_v4546 = insertelement <4 x float> %iv545_phi_vy_v4545, float %init_vy1_521, i32 1
  %iv547_phi_vy_v4547 = insertelement <4 x float> %iv546_phi_vy_v4546, float %init_vy2_523, i32 2
  %iv548_phi_vy_v4548 = insertelement <4 x float> %iv547_phi_vy_v4547, float %init_vy3_525, i32 3
  %iv549_phi_bx_v4549 = insertelement <4 x float> undef, float %init_bx0_477, i32 0
  %iv550_phi_bx_v4550 = insertelement <4 x float> %iv549_phi_bx_v4549, float %init_bx1_479, i32 1
  %iv551_phi_bx_v4551 = insertelement <4 x float> %iv550_phi_bx_v4550, float %init_bx2_481, i32 2
  %iv552_phi_bx_v4552 = insertelement <4 x float> %iv551_phi_bx_v4551, float %init_bx3_483, i32 3
  %iv553_phi_vz_v4553 = insertelement <4 x float> undef, float %init_vz0_529, i32 0
  %iv554_phi_vz_v4554 = insertelement <4 x float> %iv553_phi_vz_v4553, float %init_vz1_531, i32 1
  %iv555_phi_vz_v4555 = insertelement <4 x float> %iv554_phi_vz_v4554, float %init_vz2_533, i32 2
  %iv556_phi_vz_v4556 = insertelement <4 x float> %iv555_phi_vz_v4555, float %init_vz3_535, i32 3
  %iv557_phi_vx_v4557 = insertelement <4 x float> undef, float %init_vx0_509, i32 0
  %iv558_phi_vx_v4558 = insertelement <4 x float> %iv557_phi_vx_v4557, float %init_vx1_511, i32 1
  %iv559_phi_vx_v4559 = insertelement <4 x float> %iv558_phi_vx_v4558, float %init_vx2_513, i32 2
  %iv560_phi_vx_v4560 = insertelement <4 x float> %iv559_phi_vx_v4559, float %init_vx3_515, i32 3
  %iv561_phi_bz_v4561 = insertelement <4 x float> undef, float %init_bz0_497, i32 0
  %iv562_phi_bz_v4562 = insertelement <4 x float> %iv561_phi_bz_v4561, float %init_bz1_499, i32 1
  %iv563_phi_bz_v4563 = insertelement <4 x float> %iv562_phi_bz_v4562, float %init_bz2_501, i32 2
  %iv564_phi_bz_v4564 = insertelement <4 x float> %iv563_phi_bz_v4563, float %init_bz3_503, i32 3
  br label %loop_hdr
loop_hdr:
  %pi_cnt_540 = phi i64 [ %init_count_539, %pre_phi ], [ %pn_cnt_540, %latch ]
  %phi_by4 = phi float [ %init_by4_495, %pre_phi ], [ %be_by4, %latch ]
  %phi_bx_v4 = phi <4 x float> [ %iv552_phi_bx_v4552, %pre_phi ], [ %be_bx_v4, %latch ]
  %phi_vx_v4 = phi <4 x float> [ %iv560_phi_vx_v4560, %pre_phi ], [ %be_vx_v4, %latch ]
  %phi_vy_v4 = phi <4 x float> [ %iv548_phi_vy_v4548, %pre_phi ], [ %be_vy_v4, %latch ]
  %phi_bound = phi i64 [ %init_bound_475, %pre_phi ], [ %be_bound, %latch ]
  %phi_bz_v4 = phi <4 x float> [ %iv564_phi_bz_v4564, %pre_phi ], [ %be_bz_v4, %latch ]
  %phi_bz4 = phi float [ %init_bz4_505, %pre_phi ], [ %be_bz4, %latch ]
  %phi_by_v4 = phi <4 x float> [ %iv544_phi_by_v4544, %pre_phi ], [ %be_by_v4, %latch ]
  %phi_bx4 = phi float [ %init_bx4_485, %pre_phi ], [ %be_bx4, %latch ]
  %phi_vx4 = phi float [ %init_vx4_517, %pre_phi ], [ %be_vx4, %latch ]
  %phi_vz_v4 = phi <4 x float> [ %iv556_phi_vz_v4556, %pre_phi ], [ %be_vz_v4, %latch ]
  %phi_cycle_count = phi i64 [ %init_cycle_count_507, %pre_phi ], [ %be_cycle_count, %latch ]
  %phi_vy4 = phi float [ %init_vy4_527, %pre_phi ], [ %be_vy4, %latch ]
  %phi_vz4 = phi float [ %init_vz4_537, %pre_phi ], [ %be_vz4, %latch ]
  %phi_count = phi i64 [ %init_count_539, %pre_phi ], [ %be_count, %latch ]
  %cmp_hdr_565 = icmp slt i64 %pi_cnt_540, %cnt_bound_220
  br i1 %cmp_hdr_565, label %body, label %done
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
%t569 = fsub fast float %phi_bx_e0, %phi_bx_e1
  ; let dx01 = %t569
%t573 = fsub fast float %phi_by_e0, %phi_by_e1
  ; let dy01 = %t573
%t577 = fsub fast float %phi_bz_e0, %phi_bz_e1
  ; let dz01 = %t577
%t583 = fmul fast float %t569, %t569
%t587 = fmul fast float %t573, %t573
%t588 = fadd fast float %t583, %t587
%t592 = fmul fast float %t577, %t577
%t593 = fadd fast float %t588, %t592
  ; let dsq01 = %t593
  %t594 = call float @llvm.sqrt.f32(float %t593)
  ; let dist01 = %t594
  %il598 = load float, float* @dt, align 4
%t602 = fmul fast float %t593, %t594
%t603 = fdiv fast float %il598, %t602
  ; let mag01 = %t603
%t607 = fsub fast float %phi_bx_e0, %phi_bx_e2
  ; let dx02 = %t607
%t611 = fsub fast float %phi_by_e0, %phi_by_e2
  ; let dy02 = %t611
%t615 = fsub fast float %phi_bz_e0, %phi_bz_e2
  ; let dz02 = %t615
%t621 = fmul fast float %t607, %t607
%t625 = fmul fast float %t611, %t611
%t626 = fadd fast float %t621, %t625
%t630 = fmul fast float %t615, %t615
%t631 = fadd fast float %t626, %t630
  ; let dsq02 = %t631
  %t632 = call float @llvm.sqrt.f32(float %t631)
  ; let dist02 = %t632
  %il636 = load float, float* @dt, align 4
%t640 = fmul fast float %t631, %t632
%t641 = fdiv fast float %il636, %t640
  ; let mag02 = %t641
%t645 = fsub fast float %phi_bx_e0, %phi_bx_e3
  ; let dx03 = %t645
%t649 = fsub fast float %phi_by_e0, %phi_by_e3
  ; let dy03 = %t649
%t653 = fsub fast float %phi_bz_e0, %phi_bz_e3
  ; let dz03 = %t653
%t659 = fmul fast float %t645, %t645
%t663 = fmul fast float %t649, %t649
%t664 = fadd fast float %t659, %t663
%t668 = fmul fast float %t653, %t653
%t669 = fadd fast float %t664, %t668
  ; let dsq03 = %t669
  %t670 = call float @llvm.sqrt.f32(float %t669)
  ; let dist03 = %t670
  %il674 = load float, float* @dt, align 4
%t678 = fmul fast float %t669, %t670
%t679 = fdiv fast float %il674, %t678
  ; let mag03 = %t679
%t683 = fsub fast float %phi_bx_e0, %phi_bx4
  ; let dx04 = %t683
%t687 = fsub fast float %phi_by_e0, %phi_by4
  ; let dy04 = %t687
%t691 = fsub fast float %phi_bz_e0, %phi_bz4
  ; let dz04 = %t691
%t697 = fmul fast float %t683, %t683
%t701 = fmul fast float %t687, %t687
%t702 = fadd fast float %t697, %t701
%t706 = fmul fast float %t691, %t691
%t707 = fadd fast float %t702, %t706
  ; let dsq04 = %t707
  %t708 = call float @llvm.sqrt.f32(float %t707)
  ; let dist04 = %t708
  %il712 = load float, float* @dt, align 4
%t716 = fmul fast float %t707, %t708
%t717 = fdiv fast float %il712, %t716
  ; let mag04 = %t717
%t721 = fsub fast float %phi_bx_e1, %phi_bx_e2
  ; let dx12 = %t721
%t725 = fsub fast float %phi_by_e1, %phi_by_e2
  ; let dy12 = %t725
%t729 = fsub fast float %phi_bz_e1, %phi_bz_e2
  ; let dz12 = %t729
%t735 = fmul fast float %t721, %t721
%t739 = fmul fast float %t725, %t725
%t740 = fadd fast float %t735, %t739
%t744 = fmul fast float %t729, %t729
%t745 = fadd fast float %t740, %t744
  ; let dsq12 = %t745
  %t746 = call float @llvm.sqrt.f32(float %t745)
  ; let dist12 = %t746
  %il750 = load float, float* @dt, align 4
%t754 = fmul fast float %t745, %t746
%t755 = fdiv fast float %il750, %t754
  ; let mag12 = %t755
%t759 = fsub fast float %phi_bx_e1, %phi_bx_e3
  ; let dx13 = %t759
%t763 = fsub fast float %phi_by_e1, %phi_by_e3
  ; let dy13 = %t763
%t767 = fsub fast float %phi_bz_e1, %phi_bz_e3
  ; let dz13 = %t767
%t773 = fmul fast float %t759, %t759
%t777 = fmul fast float %t763, %t763
%t778 = fadd fast float %t773, %t777
%t782 = fmul fast float %t767, %t767
%t783 = fadd fast float %t778, %t782
  ; let dsq13 = %t783
  %t784 = call float @llvm.sqrt.f32(float %t783)
  ; let dist13 = %t784
  %il788 = load float, float* @dt, align 4
%t792 = fmul fast float %t783, %t784
%t793 = fdiv fast float %il788, %t792
  ; let mag13 = %t793
%t797 = fsub fast float %phi_bx_e1, %phi_bx4
  ; let dx14 = %t797
%t801 = fsub fast float %phi_by_e1, %phi_by4
  ; let dy14 = %t801
%t805 = fsub fast float %phi_bz_e1, %phi_bz4
  ; let dz14 = %t805
%t811 = fmul fast float %t797, %t797
%t815 = fmul fast float %t801, %t801
%t816 = fadd fast float %t811, %t815
%t820 = fmul fast float %t805, %t805
%t821 = fadd fast float %t816, %t820
  ; let dsq14 = %t821
  %t822 = call float @llvm.sqrt.f32(float %t821)
  ; let dist14 = %t822
  %il826 = load float, float* @dt, align 4
%t830 = fmul fast float %t821, %t822
%t831 = fdiv fast float %il826, %t830
  ; let mag14 = %t831
%t835 = fsub fast float %phi_bx_e2, %phi_bx_e3
  ; let dx23 = %t835
%t839 = fsub fast float %phi_by_e2, %phi_by_e3
  ; let dy23 = %t839
%t843 = fsub fast float %phi_bz_e2, %phi_bz_e3
  ; let dz23 = %t843
%t849 = fmul fast float %t835, %t835
%t853 = fmul fast float %t839, %t839
%t854 = fadd fast float %t849, %t853
%t858 = fmul fast float %t843, %t843
%t859 = fadd fast float %t854, %t858
  ; let dsq23 = %t859
  %t860 = call float @llvm.sqrt.f32(float %t859)
  ; let dist23 = %t860
  %il864 = load float, float* @dt, align 4
%t868 = fmul fast float %t859, %t860
%t869 = fdiv fast float %il864, %t868
  ; let mag23 = %t869
%t873 = fsub fast float %phi_bx_e2, %phi_bx4
  ; let dx24 = %t873
%t877 = fsub fast float %phi_by_e2, %phi_by4
  ; let dy24 = %t877
%t881 = fsub fast float %phi_bz_e2, %phi_bz4
  ; let dz24 = %t881
%t887 = fmul fast float %t873, %t873
%t891 = fmul fast float %t877, %t877
%t892 = fadd fast float %t887, %t891
%t896 = fmul fast float %t881, %t881
%t897 = fadd fast float %t892, %t896
  ; let dsq24 = %t897
  %t898 = call float @llvm.sqrt.f32(float %t897)
  ; let dist24 = %t898
  %il902 = load float, float* @dt, align 4
%t906 = fmul fast float %t897, %t898
%t907 = fdiv fast float %il902, %t906
  ; let mag24 = %t907
%t911 = fsub fast float %phi_bx_e3, %phi_bx4
  ; let dx34 = %t911
%t915 = fsub fast float %phi_by_e3, %phi_by4
  ; let dy34 = %t915
%t919 = fsub fast float %phi_bz_e3, %phi_bz4
  ; let dz34 = %t919
%t925 = fmul fast float %t911, %t911
%t929 = fmul fast float %t915, %t915
%t930 = fadd fast float %t925, %t929
%t934 = fmul fast float %t919, %t919
%t935 = fadd fast float %t930, %t934
  ; let dsq34 = %t935
  %t936 = call float @llvm.sqrt.f32(float %t935)
  ; let dist34 = %t936
  %il940 = load float, float* @dt, align 4
%t944 = fmul fast float %t935, %t936
%t945 = fdiv fast float %il940, %t944
  ; let mag34 = %t945
  %il955 = load float, float* @m1, align 4
%t956 = fmul fast float %t569, %il955
%t958 = fmul fast float %t956, %t603
%t959 = fsub fast float %phi_vx_e0, %t958
  %il964 = load float, float* @m2, align 4
%t965 = fmul fast float %t607, %il964
%t967 = fmul fast float %t965, %t641
%t968 = fsub fast float %t959, %t967
  %il973 = load float, float* @m3, align 4
%t974 = fmul fast float %t645, %il973
%t976 = fmul fast float %t974, %t679
%t977 = fsub fast float %t968, %t976
  %il982 = load float, float* @m4, align 4
%t983 = fmul fast float %t683, %il982
%t985 = fmul fast float %t983, %t717
%t986 = fsub fast float %t977, %t985
  ; let nvx0 = %t986
  %il996 = load float, float* @m1, align 4
%t997 = fmul fast float %t573, %il996
%t999 = fmul fast float %t997, %t603
%t1000 = fsub fast float %phi_vy_e0, %t999
  %il1005 = load float, float* @m2, align 4
%t1006 = fmul fast float %t611, %il1005
%t1008 = fmul fast float %t1006, %t641
%t1009 = fsub fast float %t1000, %t1008
  %il1014 = load float, float* @m3, align 4
%t1015 = fmul fast float %t649, %il1014
%t1017 = fmul fast float %t1015, %t679
%t1018 = fsub fast float %t1009, %t1017
  %il1023 = load float, float* @m4, align 4
%t1024 = fmul fast float %t687, %il1023
%t1026 = fmul fast float %t1024, %t717
%t1027 = fsub fast float %t1018, %t1026
  ; let nvy0 = %t1027
  %il1037 = load float, float* @m1, align 4
%t1038 = fmul fast float %t577, %il1037
%t1040 = fmul fast float %t1038, %t603
%t1041 = fsub fast float %phi_vz_e0, %t1040
  %il1046 = load float, float* @m2, align 4
%t1047 = fmul fast float %t615, %il1046
%t1049 = fmul fast float %t1047, %t641
%t1050 = fsub fast float %t1041, %t1049
  %il1055 = load float, float* @m3, align 4
%t1056 = fmul fast float %t653, %il1055
%t1058 = fmul fast float %t1056, %t679
%t1059 = fsub fast float %t1050, %t1058
  %il1064 = load float, float* @m4, align 4
%t1065 = fmul fast float %t691, %il1064
%t1067 = fmul fast float %t1065, %t717
%t1068 = fsub fast float %t1059, %t1067
  ; let nvz0 = %t1068
  %il1078 = load float, float* @m0, align 4
%t1079 = fmul fast float %t569, %il1078
%t1081 = fmul fast float %t1079, %t603
%t1082 = fadd fast float %phi_vx_e1, %t1081
  %il1087 = load float, float* @m2, align 4
%t1088 = fmul fast float %t721, %il1087
%t1090 = fmul fast float %t1088, %t755
%t1091 = fsub fast float %t1082, %t1090
  %il1096 = load float, float* @m3, align 4
%t1097 = fmul fast float %t759, %il1096
%t1099 = fmul fast float %t1097, %t793
%t1100 = fsub fast float %t1091, %t1099
  %il1105 = load float, float* @m4, align 4
%t1106 = fmul fast float %t797, %il1105
%t1108 = fmul fast float %t1106, %t831
%t1109 = fsub fast float %t1100, %t1108
  ; let nvx1 = %t1109
  %il1119 = load float, float* @m0, align 4
%t1120 = fmul fast float %t573, %il1119
%t1122 = fmul fast float %t1120, %t603
%t1123 = fadd fast float %phi_vy_e1, %t1122
  %il1128 = load float, float* @m2, align 4
%t1129 = fmul fast float %t725, %il1128
%t1131 = fmul fast float %t1129, %t755
%t1132 = fsub fast float %t1123, %t1131
  %il1137 = load float, float* @m3, align 4
%t1138 = fmul fast float %t763, %il1137
%t1140 = fmul fast float %t1138, %t793
%t1141 = fsub fast float %t1132, %t1140
  %il1146 = load float, float* @m4, align 4
%t1147 = fmul fast float %t801, %il1146
%t1149 = fmul fast float %t1147, %t831
%t1150 = fsub fast float %t1141, %t1149
  ; let nvy1 = %t1150
  %il1160 = load float, float* @m0, align 4
%t1161 = fmul fast float %t577, %il1160
%t1163 = fmul fast float %t1161, %t603
%t1164 = fadd fast float %phi_vz_e1, %t1163
  %il1169 = load float, float* @m2, align 4
%t1170 = fmul fast float %t729, %il1169
%t1172 = fmul fast float %t1170, %t755
%t1173 = fsub fast float %t1164, %t1172
  %il1178 = load float, float* @m3, align 4
%t1179 = fmul fast float %t767, %il1178
%t1181 = fmul fast float %t1179, %t793
%t1182 = fsub fast float %t1173, %t1181
  %il1187 = load float, float* @m4, align 4
%t1188 = fmul fast float %t805, %il1187
%t1190 = fmul fast float %t1188, %t831
%t1191 = fsub fast float %t1182, %t1190
  ; let nvz1 = %t1191
  %il1201 = load float, float* @m0, align 4
%t1202 = fmul fast float %t607, %il1201
%t1204 = fmul fast float %t1202, %t641
%t1205 = fadd fast float %phi_vx_e2, %t1204
  %il1210 = load float, float* @m1, align 4
%t1211 = fmul fast float %t721, %il1210
%t1213 = fmul fast float %t1211, %t755
%t1214 = fadd fast float %t1205, %t1213
  %il1219 = load float, float* @m3, align 4
%t1220 = fmul fast float %t835, %il1219
%t1222 = fmul fast float %t1220, %t869
%t1223 = fsub fast float %t1214, %t1222
  %il1228 = load float, float* @m4, align 4
%t1229 = fmul fast float %t873, %il1228
%t1231 = fmul fast float %t1229, %t907
%t1232 = fsub fast float %t1223, %t1231
  ; let nvx2 = %t1232
  %il1242 = load float, float* @m0, align 4
%t1243 = fmul fast float %t611, %il1242
%t1245 = fmul fast float %t1243, %t641
%t1246 = fadd fast float %phi_vy_e2, %t1245
  %il1251 = load float, float* @m1, align 4
%t1252 = fmul fast float %t725, %il1251
%t1254 = fmul fast float %t1252, %t755
%t1255 = fadd fast float %t1246, %t1254
  %il1260 = load float, float* @m3, align 4
%t1261 = fmul fast float %t839, %il1260
%t1263 = fmul fast float %t1261, %t869
%t1264 = fsub fast float %t1255, %t1263
  %il1269 = load float, float* @m4, align 4
%t1270 = fmul fast float %t877, %il1269
%t1272 = fmul fast float %t1270, %t907
%t1273 = fsub fast float %t1264, %t1272
  ; let nvy2 = %t1273
  %il1283 = load float, float* @m0, align 4
%t1284 = fmul fast float %t615, %il1283
%t1286 = fmul fast float %t1284, %t641
%t1287 = fadd fast float %phi_vz_e2, %t1286
  %il1292 = load float, float* @m1, align 4
%t1293 = fmul fast float %t729, %il1292
%t1295 = fmul fast float %t1293, %t755
%t1296 = fadd fast float %t1287, %t1295
  %il1301 = load float, float* @m3, align 4
%t1302 = fmul fast float %t843, %il1301
%t1304 = fmul fast float %t1302, %t869
%t1305 = fsub fast float %t1296, %t1304
  %il1310 = load float, float* @m4, align 4
%t1311 = fmul fast float %t881, %il1310
%t1313 = fmul fast float %t1311, %t907
%t1314 = fsub fast float %t1305, %t1313
  ; let nvz2 = %t1314
  %il1324 = load float, float* @m0, align 4
%t1325 = fmul fast float %t645, %il1324
%t1327 = fmul fast float %t1325, %t679
%t1328 = fadd fast float %phi_vx_e3, %t1327
  %il1333 = load float, float* @m1, align 4
%t1334 = fmul fast float %t759, %il1333
%t1336 = fmul fast float %t1334, %t793
%t1337 = fadd fast float %t1328, %t1336
  %il1342 = load float, float* @m2, align 4
%t1343 = fmul fast float %t835, %il1342
%t1345 = fmul fast float %t1343, %t869
%t1346 = fadd fast float %t1337, %t1345
  %il1351 = load float, float* @m4, align 4
%t1352 = fmul fast float %t911, %il1351
%t1354 = fmul fast float %t1352, %t945
%t1355 = fsub fast float %t1346, %t1354
  ; let nvx3 = %t1355
  %il1365 = load float, float* @m0, align 4
%t1366 = fmul fast float %t649, %il1365
%t1368 = fmul fast float %t1366, %t679
%t1369 = fadd fast float %phi_vy_e3, %t1368
  %il1374 = load float, float* @m1, align 4
%t1375 = fmul fast float %t763, %il1374
%t1377 = fmul fast float %t1375, %t793
%t1378 = fadd fast float %t1369, %t1377
  %il1383 = load float, float* @m2, align 4
%t1384 = fmul fast float %t839, %il1383
%t1386 = fmul fast float %t1384, %t869
%t1387 = fadd fast float %t1378, %t1386
  %il1392 = load float, float* @m4, align 4
%t1393 = fmul fast float %t911, %il1392
%t1395 = fmul fast float %t1393, %t945
%t1396 = fsub fast float %t1387, %t1395
  ; let nvy3 = %t1396
  %il1406 = load float, float* @m0, align 4
%t1407 = fmul fast float %t653, %il1406
%t1409 = fmul fast float %t1407, %t679
%t1410 = fadd fast float %phi_vz_e3, %t1409
  %il1415 = load float, float* @m1, align 4
%t1416 = fmul fast float %t767, %il1415
%t1418 = fmul fast float %t1416, %t793
%t1419 = fadd fast float %t1410, %t1418
  %il1424 = load float, float* @m2, align 4
%t1425 = fmul fast float %t843, %il1424
%t1427 = fmul fast float %t1425, %t869
%t1428 = fadd fast float %t1419, %t1427
  %il1433 = load float, float* @m4, align 4
%t1434 = fmul fast float %t919, %il1433
%t1436 = fmul fast float %t1434, %t945
%t1437 = fsub fast float %t1428, %t1436
  ; let nvz3 = %t1437
  %il1447 = load float, float* @m0, align 4
%t1448 = fmul fast float %t683, %il1447
%t1450 = fmul fast float %t1448, %t717
%t1451 = fadd fast float %phi_vx4, %t1450
  %il1456 = load float, float* @m1, align 4
%t1457 = fmul fast float %t797, %il1456
%t1459 = fmul fast float %t1457, %t831
%t1460 = fadd fast float %t1451, %t1459
  %il1465 = load float, float* @m2, align 4
%t1466 = fmul fast float %t873, %il1465
%t1468 = fmul fast float %t1466, %t907
%t1469 = fadd fast float %t1460, %t1468
  %il1474 = load float, float* @m3, align 4
%t1475 = fmul fast float %t911, %il1474
%t1477 = fmul fast float %t1475, %t945
%t1478 = fadd fast float %t1469, %t1477
  ; let nvx4 = %t1478
  %il1488 = load float, float* @m0, align 4
%t1489 = fmul fast float %t687, %il1488
%t1491 = fmul fast float %t1489, %t717
%t1492 = fadd fast float %phi_vy4, %t1491
  %il1497 = load float, float* @m1, align 4
%t1498 = fmul fast float %t801, %il1497
%t1500 = fmul fast float %t1498, %t831
%t1501 = fadd fast float %t1492, %t1500
  %il1506 = load float, float* @m2, align 4
%t1507 = fmul fast float %t877, %il1506
%t1509 = fmul fast float %t1507, %t907
%t1510 = fadd fast float %t1501, %t1509
  %il1515 = load float, float* @m3, align 4
%t1516 = fmul fast float %t915, %il1515
%t1518 = fmul fast float %t1516, %t945
%t1519 = fadd fast float %t1510, %t1518
  ; let nvy4 = %t1519
  %il1529 = load float, float* @m0, align 4
%t1530 = fmul fast float %t691, %il1529
%t1532 = fmul fast float %t1530, %t717
%t1533 = fadd fast float %phi_vz4, %t1532
  %il1538 = load float, float* @m1, align 4
%t1539 = fmul fast float %t805, %il1538
%t1541 = fmul fast float %t1539, %t831
%t1542 = fadd fast float %t1533, %t1541
  %il1547 = load float, float* @m2, align 4
%t1548 = fmul fast float %t881, %il1547
%t1550 = fmul fast float %t1548, %t907
%t1551 = fadd fast float %t1542, %t1550
  %il1556 = load float, float* @m3, align 4
%t1557 = fmul fast float %t919, %il1556
%t1559 = fmul fast float %t1557, %t945
%t1560 = fadd fast float %t1551, %t1559
  ; let nvz4 = %t1560
   %iv1562_phi_vx_v4 = insertelement <4 x float> %phi_vx_v4, float %t986, i32 0
   %iv1564_phi_vy_v4 = insertelement <4 x float> %phi_vy_v4, float %t1027, i32 0
   %iv1566_phi_vz_v4 = insertelement <4 x float> %phi_vz_v4, float %t1068, i32 0
   %iv1568_phi_vx_v4 = insertelement <4 x float> %iv1562_phi_vx_v4, float %t1109, i32 1
   %iv1570_phi_vy_v4 = insertelement <4 x float> %iv1564_phi_vy_v4, float %t1150, i32 1
   %iv1572_phi_vz_v4 = insertelement <4 x float> %iv1566_phi_vz_v4, float %t1191, i32 1
   %iv1574_phi_vx_v4 = insertelement <4 x float> %iv1568_phi_vx_v4, float %t1232, i32 2
   %iv1576_phi_vy_v4 = insertelement <4 x float> %iv1570_phi_vy_v4, float %t1273, i32 2
   %iv1578_phi_vz_v4 = insertelement <4 x float> %iv1572_phi_vz_v4, float %t1314, i32 2
   %iv1580_phi_vx_v4 = insertelement <4 x float> %iv1574_phi_vx_v4, float %t1355, i32 3
   %iv1582_phi_vy_v4 = insertelement <4 x float> %iv1576_phi_vy_v4, float %t1396, i32 3
   %iv1584_phi_vz_v4 = insertelement <4 x float> %iv1578_phi_vz_v4, float %t1437, i32 3
  %ap_1586 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 14
  %ap_1588 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 0
  %ap_1590 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 1
  %il1595 = load float, float* @dt, align 4
%t1597 = fmul fast float %il1595, %t986
%t1598 = fadd fast float %phi_bx_e0, %t1597
   %iv1599_phi_bx_v4 = insertelement <4 x float> %phi_bx_v4, float %t1598, i32 0
  %il1604 = load float, float* @dt, align 4
%t1606 = fmul fast float %il1604, %t1027
%t1607 = fadd fast float %phi_by_e0, %t1606
   %iv1608_phi_by_v4 = insertelement <4 x float> %phi_by_v4, float %t1607, i32 0
  %il1613 = load float, float* @dt, align 4
%t1615 = fmul fast float %il1613, %t1068
%t1616 = fadd fast float %phi_bz_e0, %t1615
   %iv1617_phi_bz_v4 = insertelement <4 x float> %phi_bz_v4, float %t1616, i32 0
  %il1622 = load float, float* @dt, align 4
%t1624 = fmul fast float %il1622, %t1109
%t1625 = fadd fast float %phi_bx_e1, %t1624
   %iv1626_phi_bx_v4 = insertelement <4 x float> %iv1599_phi_bx_v4, float %t1625, i32 1
  %il1631 = load float, float* @dt, align 4
%t1633 = fmul fast float %il1631, %t1150
%t1634 = fadd fast float %phi_by_e1, %t1633
   %iv1635_phi_by_v4 = insertelement <4 x float> %iv1608_phi_by_v4, float %t1634, i32 1
  %il1640 = load float, float* @dt, align 4
%t1642 = fmul fast float %il1640, %t1191
%t1643 = fadd fast float %phi_bz_e1, %t1642
   %iv1644_phi_bz_v4 = insertelement <4 x float> %iv1617_phi_bz_v4, float %t1643, i32 1
  %il1649 = load float, float* @dt, align 4
%t1651 = fmul fast float %il1649, %t1232
%t1652 = fadd fast float %phi_bx_e2, %t1651
   %iv1653_phi_bx_v4 = insertelement <4 x float> %iv1626_phi_bx_v4, float %t1652, i32 2
  %il1658 = load float, float* @dt, align 4
%t1660 = fmul fast float %il1658, %t1273
%t1661 = fadd fast float %phi_by_e2, %t1660
   %iv1662_phi_by_v4 = insertelement <4 x float> %iv1635_phi_by_v4, float %t1661, i32 2
  %il1667 = load float, float* @dt, align 4
%t1669 = fmul fast float %il1667, %t1314
%t1670 = fadd fast float %phi_bz_e2, %t1669
   %iv1671_phi_bz_v4 = insertelement <4 x float> %iv1644_phi_bz_v4, float %t1670, i32 2
  %il1676 = load float, float* @dt, align 4
%t1678 = fmul fast float %il1676, %t1355
%t1679 = fadd fast float %phi_bx_e3, %t1678
   %iv1680_phi_bx_v4 = insertelement <4 x float> %iv1653_phi_bx_v4, float %t1679, i32 3
  %il1685 = load float, float* @dt, align 4
%t1687 = fmul fast float %il1685, %t1396
%t1688 = fadd fast float %phi_by_e3, %t1687
   %iv1689_phi_by_v4 = insertelement <4 x float> %iv1662_phi_by_v4, float %t1688, i32 3
  %il1694 = load float, float* @dt, align 4
%t1696 = fmul fast float %il1694, %t1437
%t1697 = fadd fast float %phi_bz_e3, %t1696
   %iv1698_phi_bz_v4 = insertelement <4 x float> %iv1671_phi_bz_v4, float %t1697, i32 3
  %il1703 = load float, float* @dt, align 4
%t1705 = fmul fast float %il1703, %t1478
%t1706 = fadd fast float %phi_bx4, %t1705
  %ap_1707 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 11
  %il1712 = load float, float* @dt, align 4
%t1714 = fmul fast float %il1712, %t1519
%t1715 = fadd fast float %phi_by4, %t1714
  %ap_1716 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 12
  %il1721 = load float, float* @dt, align 4
%t1723 = fmul fast float %il1721, %t1560
%t1724 = fadd fast float %phi_bz4, %t1723
  %ap_1725 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 13
%t1733 = fsub fast float %phi_bx_e0, %phi_bx_e1
%t1737 = fsub fast float %phi_bx_e0, %phi_bx_e1
%t1738 = fmul fast float %t1733, %t1737
%t1743 = fsub fast float %phi_by_e0, %phi_by_e1
%t1747 = fsub fast float %phi_by_e0, %phi_by_e1
%t1748 = fmul fast float %t1743, %t1747
%t1749 = fadd fast float %t1738, %t1748
%t1754 = fsub fast float %phi_bz_e0, %phi_bz_e1
%t1758 = fsub fast float %phi_bz_e0, %phi_bz_e1
%t1759 = fmul fast float %t1754, %t1758
%t1760 = fadd fast float %t1749, %t1759
  %t1726 = call float @llvm.sqrt.f32(float %t1760)
  ; let edist01 = %t1726
%t1768 = fsub fast float %phi_bx_e0, %phi_bx_e2
%t1772 = fsub fast float %phi_bx_e0, %phi_bx_e2
%t1773 = fmul fast float %t1768, %t1772
%t1778 = fsub fast float %phi_by_e0, %phi_by_e2
%t1782 = fsub fast float %phi_by_e0, %phi_by_e2
%t1783 = fmul fast float %t1778, %t1782
%t1784 = fadd fast float %t1773, %t1783
%t1789 = fsub fast float %phi_bz_e0, %phi_bz_e2
%t1793 = fsub fast float %phi_bz_e0, %phi_bz_e2
%t1794 = fmul fast float %t1789, %t1793
%t1795 = fadd fast float %t1784, %t1794
  %t1761 = call float @llvm.sqrt.f32(float %t1795)
  ; let edist02 = %t1761
%t1803 = fsub fast float %phi_bx_e0, %phi_bx_e3
%t1807 = fsub fast float %phi_bx_e0, %phi_bx_e3
%t1808 = fmul fast float %t1803, %t1807
%t1813 = fsub fast float %phi_by_e0, %phi_by_e3
%t1817 = fsub fast float %phi_by_e0, %phi_by_e3
%t1818 = fmul fast float %t1813, %t1817
%t1819 = fadd fast float %t1808, %t1818
%t1824 = fsub fast float %phi_bz_e0, %phi_bz_e3
%t1828 = fsub fast float %phi_bz_e0, %phi_bz_e3
%t1829 = fmul fast float %t1824, %t1828
%t1830 = fadd fast float %t1819, %t1829
  %t1796 = call float @llvm.sqrt.f32(float %t1830)
  ; let edist03 = %t1796
%t1838 = fsub fast float %phi_bx_e0, %phi_bx4
%t1842 = fsub fast float %phi_bx_e0, %phi_bx4
%t1843 = fmul fast float %t1838, %t1842
%t1848 = fsub fast float %phi_by_e0, %phi_by4
%t1852 = fsub fast float %phi_by_e0, %phi_by4
%t1853 = fmul fast float %t1848, %t1852
%t1854 = fadd fast float %t1843, %t1853
%t1859 = fsub fast float %phi_bz_e0, %phi_bz4
%t1863 = fsub fast float %phi_bz_e0, %phi_bz4
%t1864 = fmul fast float %t1859, %t1863
%t1865 = fadd fast float %t1854, %t1864
  %t1831 = call float @llvm.sqrt.f32(float %t1865)
  ; let edist04 = %t1831
%t1873 = fsub fast float %phi_bx_e1, %phi_bx_e2
%t1877 = fsub fast float %phi_bx_e1, %phi_bx_e2
%t1878 = fmul fast float %t1873, %t1877
%t1883 = fsub fast float %phi_by_e1, %phi_by_e2
%t1887 = fsub fast float %phi_by_e1, %phi_by_e2
%t1888 = fmul fast float %t1883, %t1887
%t1889 = fadd fast float %t1878, %t1888
%t1894 = fsub fast float %phi_bz_e1, %phi_bz_e2
%t1898 = fsub fast float %phi_bz_e1, %phi_bz_e2
%t1899 = fmul fast float %t1894, %t1898
%t1900 = fadd fast float %t1889, %t1899
  %t1866 = call float @llvm.sqrt.f32(float %t1900)
  ; let edist12 = %t1866
%t1908 = fsub fast float %phi_bx_e1, %phi_bx_e3
%t1912 = fsub fast float %phi_bx_e1, %phi_bx_e3
%t1913 = fmul fast float %t1908, %t1912
%t1918 = fsub fast float %phi_by_e1, %phi_by_e3
%t1922 = fsub fast float %phi_by_e1, %phi_by_e3
%t1923 = fmul fast float %t1918, %t1922
%t1924 = fadd fast float %t1913, %t1923
%t1929 = fsub fast float %phi_bz_e1, %phi_bz_e3
%t1933 = fsub fast float %phi_bz_e1, %phi_bz_e3
%t1934 = fmul fast float %t1929, %t1933
%t1935 = fadd fast float %t1924, %t1934
  %t1901 = call float @llvm.sqrt.f32(float %t1935)
  ; let edist13 = %t1901
%t1943 = fsub fast float %phi_bx_e1, %phi_bx4
%t1947 = fsub fast float %phi_bx_e1, %phi_bx4
%t1948 = fmul fast float %t1943, %t1947
%t1953 = fsub fast float %phi_by_e1, %phi_by4
%t1957 = fsub fast float %phi_by_e1, %phi_by4
%t1958 = fmul fast float %t1953, %t1957
%t1959 = fadd fast float %t1948, %t1958
%t1964 = fsub fast float %phi_bz_e1, %phi_bz4
%t1968 = fsub fast float %phi_bz_e1, %phi_bz4
%t1969 = fmul fast float %t1964, %t1968
%t1970 = fadd fast float %t1959, %t1969
  %t1936 = call float @llvm.sqrt.f32(float %t1970)
  ; let edist14 = %t1936
%t1978 = fsub fast float %phi_bx_e2, %phi_bx_e3
%t1982 = fsub fast float %phi_bx_e2, %phi_bx_e3
%t1983 = fmul fast float %t1978, %t1982
%t1988 = fsub fast float %phi_by_e2, %phi_by_e3
%t1992 = fsub fast float %phi_by_e2, %phi_by_e3
%t1993 = fmul fast float %t1988, %t1992
%t1994 = fadd fast float %t1983, %t1993
%t1999 = fsub fast float %phi_bz_e2, %phi_bz_e3
%t2003 = fsub fast float %phi_bz_e2, %phi_bz_e3
%t2004 = fmul fast float %t1999, %t2003
%t2005 = fadd fast float %t1994, %t2004
  %t1971 = call float @llvm.sqrt.f32(float %t2005)
  ; let edist23 = %t1971
%t2013 = fsub fast float %phi_bx_e2, %phi_bx4
%t2017 = fsub fast float %phi_bx_e2, %phi_bx4
%t2018 = fmul fast float %t2013, %t2017
%t2023 = fsub fast float %phi_by_e2, %phi_by4
%t2027 = fsub fast float %phi_by_e2, %phi_by4
%t2028 = fmul fast float %t2023, %t2027
%t2029 = fadd fast float %t2018, %t2028
%t2034 = fsub fast float %phi_bz_e2, %phi_bz4
%t2038 = fsub fast float %phi_bz_e2, %phi_bz4
%t2039 = fmul fast float %t2034, %t2038
%t2040 = fadd fast float %t2029, %t2039
  %t2006 = call float @llvm.sqrt.f32(float %t2040)
  ; let edist24 = %t2006
%t2048 = fsub fast float %phi_bx_e3, %phi_bx4
%t2052 = fsub fast float %phi_bx_e3, %phi_bx4
%t2053 = fmul fast float %t2048, %t2052
%t2058 = fsub fast float %phi_by_e3, %phi_by4
%t2062 = fsub fast float %phi_by_e3, %phi_by4
%t2063 = fmul fast float %t2058, %t2062
%t2064 = fadd fast float %t2053, %t2063
%t2069 = fsub fast float %phi_bz_e3, %phi_bz4
%t2073 = fsub fast float %phi_bz_e3, %phi_bz4
%t2074 = fmul fast float %t2069, %t2073
%t2075 = fadd fast float %t2064, %t2074
  %t2041 = call float @llvm.sqrt.f32(float %t2075)
  ; let edist34 = %t2041
  %il2079 = load float, float* @m0, align 4
  %il2081 = load float, float* @m1, align 4
%t2082 = fmul fast float %il2079, %il2081
%t2084 = fdiv fast float %t2082, %t1726
  ; let e01 = %t2084
  %il2088 = load float, float* @m0, align 4
  %il2090 = load float, float* @m2, align 4
%t2091 = fmul fast float %il2088, %il2090
%t2093 = fdiv fast float %t2091, %t1761
  ; let e02 = %t2093
  %il2097 = load float, float* @m0, align 4
  %il2099 = load float, float* @m3, align 4
%t2100 = fmul fast float %il2097, %il2099
%t2102 = fdiv fast float %t2100, %t1796
  ; let e03 = %t2102
  %il2106 = load float, float* @m0, align 4
  %il2108 = load float, float* @m4, align 4
%t2109 = fmul fast float %il2106, %il2108
%t2111 = fdiv fast float %t2109, %t1831
  ; let e04 = %t2111
  %il2115 = load float, float* @m1, align 4
  %il2117 = load float, float* @m2, align 4
%t2118 = fmul fast float %il2115, %il2117
%t2120 = fdiv fast float %t2118, %t1866
  ; let e12 = %t2120
  %il2124 = load float, float* @m1, align 4
  %il2126 = load float, float* @m3, align 4
%t2127 = fmul fast float %il2124, %il2126
%t2129 = fdiv fast float %t2127, %t1901
  ; let e13 = %t2129
  %il2133 = load float, float* @m1, align 4
  %il2135 = load float, float* @m4, align 4
%t2136 = fmul fast float %il2133, %il2135
%t2138 = fdiv fast float %t2136, %t1936
  ; let e14 = %t2138
  %il2142 = load float, float* @m2, align 4
  %il2144 = load float, float* @m3, align 4
%t2145 = fmul fast float %il2142, %il2144
%t2147 = fdiv fast float %t2145, %t1971
  ; let e23 = %t2147
  %il2151 = load float, float* @m2, align 4
  %il2153 = load float, float* @m4, align 4
%t2154 = fmul fast float %il2151, %il2153
%t2156 = fdiv fast float %t2154, %t2006
  ; let e24 = %t2156
  %il2160 = load float, float* @m3, align 4
  %il2162 = load float, float* @m4, align 4
%t2163 = fmul fast float %il2160, %il2162
%t2165 = fdiv fast float %t2163, %t2041
  ; let e34 = %t2165
%t2179 = fadd fast float %t2084, %t2093
%t2181 = fadd fast float %t2179, %t2102
%t2183 = fadd fast float %t2181, %t2111
%t2185 = fadd fast float %t2183, %t2120
%t2187 = fadd fast float %t2185, %t2129
%t2189 = fadd fast float %t2187, %t2138
%t2191 = fadd fast float %t2189, %t2147
%t2193 = fadd fast float %t2191, %t2156
%t2195 = fadd fast float %t2193, %t2165
  %t2196 = fneg float %t2195
  ; let ep = %t2196
%ff2201 = bitcast i32 1056964608 to float
  %il2203 = load float, float* @m0, align 4
%t2204 = fmul fast float %ff2201, %il2203
%t2210 = fmul fast float %phi_vx_e0, %phi_vx_e0
%t2214 = fmul fast float %phi_vy_e0, %phi_vy_e0
%t2215 = fadd fast float %t2210, %t2214
%t2219 = fmul fast float %phi_vz_e0, %phi_vz_e0
%t2220 = fadd fast float %t2215, %t2219
%t2221 = fmul fast float %t2204, %t2220
  ; let ek0 = %t2221
%ff2226 = bitcast i32 1056964608 to float
  %il2228 = load float, float* @m1, align 4
%t2229 = fmul fast float %ff2226, %il2228
%t2235 = fmul fast float %phi_vx_e1, %phi_vx_e1
%t2239 = fmul fast float %phi_vy_e1, %phi_vy_e1
%t2240 = fadd fast float %t2235, %t2239
%t2244 = fmul fast float %phi_vz_e1, %phi_vz_e1
%t2245 = fadd fast float %t2240, %t2244
%t2246 = fmul fast float %t2229, %t2245
  ; let ek1 = %t2246
%ff2251 = bitcast i32 1056964608 to float
  %il2253 = load float, float* @m2, align 4
%t2254 = fmul fast float %ff2251, %il2253
%t2260 = fmul fast float %phi_vx_e2, %phi_vx_e2
%t2264 = fmul fast float %phi_vy_e2, %phi_vy_e2
%t2265 = fadd fast float %t2260, %t2264
%t2269 = fmul fast float %phi_vz_e2, %phi_vz_e2
%t2270 = fadd fast float %t2265, %t2269
%t2271 = fmul fast float %t2254, %t2270
  ; let ek2 = %t2271
%ff2276 = bitcast i32 1056964608 to float
  %il2278 = load float, float* @m3, align 4
%t2279 = fmul fast float %ff2276, %il2278
%t2285 = fmul fast float %phi_vx_e3, %phi_vx_e3
%t2289 = fmul fast float %phi_vy_e3, %phi_vy_e3
%t2290 = fadd fast float %t2285, %t2289
%t2294 = fmul fast float %phi_vz_e3, %phi_vz_e3
%t2295 = fadd fast float %t2290, %t2294
%t2296 = fmul fast float %t2279, %t2295
  ; let ek3 = %t2296
%ff2301 = bitcast i32 1056964608 to float
  %il2303 = load float, float* @m4, align 4
%t2304 = fmul fast float %ff2301, %il2303
%t2310 = fmul fast float %phi_vx4, %phi_vx4
%t2314 = fmul fast float %phi_vy4, %phi_vy4
%t2315 = fadd fast float %t2310, %t2314
%t2319 = fmul fast float %phi_vz4, %phi_vz4
%t2320 = fadd fast float %t2315, %t2319
%t2321 = fmul fast float %t2304, %t2320
  ; let ek4 = %t2321
%t2329 = fadd fast float %t2196, %t2221
%t2331 = fadd fast float %t2329, %t2246
%t2333 = fadd fast float %t2331, %t2271
%t2335 = fadd fast float %t2333, %t2296
%t2337 = fadd fast float %t2335, %t2321
  ; let energy = %t2337
  %t2340 = add i64 0, %phi_count
%t2342 = add i64 0, 5000000
%t2343 = srem i64 %t2340, %t2342
%t2345 = add i64 0, 0
%c2346 = icmp eq i64 %t2343, %t2345
  br i1 %c2346, label %g2347_t, label %g2347_e
  g2347_t:
    %pfd2350 = fpext float %t2337 to double
    %pso2351 = load volatile ptr, ptr @stdout
    %pff2352 = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0
    %ppf2353 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso2351, ptr %pff2352, double %pfd2350)
    %t2348 = zext i32 %ppf2353 to i64
    ; let __printed = %t2348
    br label %g2347_tx
  g2347_tx:
    br label %g2347_e
  g2347_e:
  %t2355 = add i64 0, %phi_count
%t2357 = add i64 0, 1
%t2358 = add nsw i64 %t2355, %t2357
  %ap_2359 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
  store i64 %t2358, ptr %ap_2359, align 8, !tbaa !1
  br label %latch
latch:
  %pn_cnt_540 = add i64 %pi_cnt_540, 1
  %be_bound = add i64 0, %phi_bound
  %be_bx_v4 = bitcast <4 x float> %iv1680_phi_bx_v4 to <4 x float>
  %be_bx4 = fadd float %t1706, 0.0
  %be_by_v4 = bitcast <4 x float> %iv1689_phi_by_v4 to <4 x float>
  %be_by4 = fadd float %t1715, 0.0
  %be_bz_v4 = bitcast <4 x float> %iv1698_phi_bz_v4 to <4 x float>
  %be_bz4 = fadd float %t1724, 0.0
  %be_cycle_count = add i64 0, %phi_cycle_count
  %be_vx_v4 = bitcast <4 x float> %iv1580_phi_vx_v4 to <4 x float>
  %be_vx4 = fadd float %t1478, 0.0
  %be_vy_v4 = bitcast <4 x float> %iv1582_phi_vy_v4 to <4 x float>
  %be_vy4 = fadd float %t1519, 0.0
  %be_vz_v4 = bitcast <4 x float> %iv1584_phi_vz_v4 to <4 x float>
  %be_vz4 = fadd float %t1560, 0.0
  %be_count = add i64 0, %pn_cnt_540
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

!0 = !{!"Briv"}
!1 = !{!"Int", !0}
!2 = !{!"Char", !0}
!3 = !{!"Float", !0}
!4 = !{!"Bool", !0}
!5 = !{!"String", !0}
!99 = distinct !{} ; StateAliasScope
