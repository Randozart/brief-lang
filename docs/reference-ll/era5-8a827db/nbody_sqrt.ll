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
@pi = constant float bitcast (i32 1078530011 to float)
@dt = constant float bitcast (i32 1008981770 to float)
@solar_mass = constant float bitcast (i32 1109256678 to float)
@m4 = constant float bitcast (i32 990201755 to float)
@m0 = alias float, float* @solar_mass
@m1 = constant float bitcast (i32 1025139887 to float)
@m3 = constant float bitcast (i32 987885205 to float)
@dpy = constant float bitcast (i32 1136041656 to float)
@m2 = constant float bitcast (i32 1010362952 to float)

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
  %r = call i64@brief_open (i64 %path, i64 %flags, i64 %mode);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_close(i64 %fd) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_close (i64 %fd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_read(i64 %fd, i64 %buf, i64 %count) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_read (i64 %fd, i64 %buf, i64 %count);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_write(i64 %fd, i64 %buf, i64 %count) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_write (i64 %fd, i64 %buf, i64 %count);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_lseek(i64 %fd, i64 %offset, i64 %whence) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_lseek (i64 %fd, i64 %offset, i64 %whence);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_pread(i64 %fd, i64 %buf, i64 %count, i64 %offset) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_pread (i64 %fd, i64 %buf, i64 %count, i64 %offset);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_pwrite(i64 %fd, i64 %buf, i64 %count, i64 %offset) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_pwrite (i64 %fd, i64 %buf, i64 %count, i64 %offset);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_stat(i64 %path, i64 %buf) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_stat (i64 %path, i64 %buf);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_fstat(i64 %fd, i64 %buf) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_fstat (i64 %fd, i64 %buf);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_ftruncate(i64 %fd, i64 %length) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_ftruncate (i64 %fd, i64 %length);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_fsync(i64 %fd) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_fsync (i64 %fd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_dup(i64 %fd) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_dup (i64 %fd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_dup2(i64 %oldfd, i64 %newfd) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_dup2 (i64 %oldfd, i64 %newfd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @file_fcntl(i64 %fd, i64 %cmd, i64 %arg) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_fcntl (i64 %fd, i64 %cmd, i64 %arg);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_socket(i64 %domain, i64 %type_, i64 %protocol) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_socket (i64 %domain, i64 %type_, i64 %protocol);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_bind(i64 %fd, i64 %addr, i64 %addrlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_bind (i64 %fd, i64 %addr, i64 %addrlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_listen(i64 %fd, i64 %backlog) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_listen (i64 %fd, i64 %backlog);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_accept(i64 %fd, i64 %addr, i64 %addrlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_accept (i64 %fd, i64 %addr, i64 %addrlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_connect(i64 %fd, i64 %addr, i64 %addrlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_connect (i64 %fd, i64 %addr, i64 %addrlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_send(i64 %fd, i64 %buf, i64 %len, i64 %flags) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_send (i64 %fd, i64 %buf, i64 %len, i64 %flags);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_recv(i64 %fd, i64 %buf, i64 %len, i64 %flags) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_recv (i64 %fd, i64 %buf, i64 %len, i64 %flags);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_sendto(i64 %fd, i64 %buf, i64 %len, i64 %flags, i64 %dest, i64 %destlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_sendto (i64 %fd, i64 %buf, i64 %len, i64 %flags, i64 %dest, i64 %destlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_recvfrom(i64 %fd, i64 %buf, i64 %len, i64 %flags, i64 %src, i64 %srclen) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_recvfrom (i64 %fd, i64 %buf, i64 %len, i64 %flags, i64 %src, i64 %srclen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_setsockopt(i64 %fd, i64 %level, i64 %optname, i64 %optval, i64 %optlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_setsockopt (i64 %fd, i64 %level, i64 %optname, i64 %optval, i64 %optlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getsockopt(i64 %fd, i64 %level, i64 %optname, i64 %optval, i64 %optlen) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_getsockopt (i64 %fd, i64 %level, i64 %optname, i64 %optval, i64 %optlen);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_shutdown(i64 %fd, i64 %how) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_shutdown (i64 %fd, i64 %how);
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
  %r = call i64@brief_pipe (i64 %pipefd);
  ret i64 %r;
  ret i64 0
}

define internal i64 @shm_open(i64 %name, i64 %oflag, i64 %mode) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_shm_open (i64 %name, i64 %oflag, i64 %mode);
  ret i64 %r;
  ret i64 0
}

define internal i64 @shm_unlink(i64 %name) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_shm_unlink (i64 %name);
  ret i64 %r;
  ret i64 0
}

define internal i64 @sem_open(i64 %name, i64 %oflag, i64 %mode, i64 %value) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_sem_open (i64 %name, i64 %oflag, i64 %mode, i64 %value);
  ret i64 %r;
  ret i64 0
}

define internal i64 @sem_wait(i64 %sem) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_sem_wait (i64 %sem);
  ret i64 %r;
  ret i64 0
}

define internal i64 @sem_post(i64 %sem) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_sem_post (i64 %sem);
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
  %r = call i64@brief_mkdir (i64 %path, i64 %mode);
  ret i64 %r;
  ret i64 0
}

define internal i64 @rmdir(i64 %path) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_rmdir (i64 %path);
  ret i64 %r;
  ret i64 0
}

define internal i64 @unlink(i64 %path) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_unlink (i64 %path);
  ret i64 %r;
  ret i64 0
}

define internal i64 @rename(i64 %oldpath, i64 %newpath) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_rename (i64 %oldpath, i64 %newpath);
  ret i64 %r;
  ret i64 0
}

define internal i64 @symlink(i64 %target, i64 %linkpath) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_symlink (i64 %target, i64 %linkpath);
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
  %r = call i64@brief_link (i64 %oldpath, i64 %newpath);
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
  %r = call i64@brief_chdir (i64 %path);
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
  %r = call i64@brief_chmod (i64 %path, i64 %mode);
  ret i64 %r;
  ret i64 0
}

define internal i64 @chown(i64 %path, i64 %owner, i64 %group) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_chown (i64 %path, i64 %owner, i64 %group);
  ret i64 %r;
  ret i64 0
}

define internal i64 @umask(i64 %mask) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_umask (i64 %mask);
  ret i64 %r;
  ret i64 0
}

define internal i64 @access(i64 %path, i64 %mode) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_access (i64 %path, i64 %mode);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getpid() local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_getpid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getppid() local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_getppid ();
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
  %r = call i64@brief_getuid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_geteuid() local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_geteuid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getgid() local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_getgid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_getegid() local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_getegid ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_clock_gettime(i64 %clock_id, i64 %tp) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_clock_gettime (i64 %clock_id, i64 %tp);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_nanosleep(i64 %req, i64 %rem) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_nanosleep (i64 %req, i64 %rem);
  ret i64 %r;
  ret i64 0
}

define internal i64 @mmap(i64 %addr, i64 %length, i64 %prot, i64 %flags, i64 %fd, i64 %offset) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_mmap (i64 %addr, i64 %length, i64 %prot, i64 %flags, i64 %fd, i64 %offset);
  ret i64 %r;
  ret i64 0
}

define internal i64 @munmap(i64 %addr, i64 %length) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_munmap (i64 %addr, i64 %length);
  ret i64 %r;
  ret i64 0
}

define internal i64 @mprotect(i64 %addr, i64 %length, i64 %prot) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_mprotect (i64 %addr, i64 %length, i64 %prot);
  ret i64 %r;
  ret i64 0
}

define internal i64 @brk(i64 %addr) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_brk (i64 %addr);
  ret i64 %r;
  ret i64 0
}

define internal i64 @mlock(i64 %addr, i64 %length) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_mlock (i64 %addr, i64 %length);
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
  %r = call i64@brief_sched_yield ();
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
  %r = call i64@brief_pagesize ();
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_cpu_count() local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_cpu_count ();
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
  %r = call i64@brief_ring_push (i64 %handle, i64 %val);
  ret i64 %r;
  ret i64 0
}

define internal i64 @__sys_ring_pop(i64 %handle) local_unnamed_addr #0 {
  entry:
  %r = call i64@brief_ring_pop (i64 %handle);
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
  %r = call i64@brief_futex (i64 %uaddr, i64 %opcode, i64 %val, i64 %timeout, i64 %uaddr2, i64 %val3);
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
  %t356 = call float @llvm.sqrt.f32(float %t211)
  ; let dist01 = %t356
  %t358 = call float @llvm.sqrt.f32(float %t227)
  ; let dist02 = %t358
  %t360 = call float @llvm.sqrt.f32(float %t243)
  ; let dist03 = %t360
  %t362 = call float @llvm.sqrt.f32(float %t259)
  ; let dist04 = %t362
  %t364 = call float @llvm.sqrt.f32(float %t275)
  ; let dist12 = %t364
  %t366 = call float @llvm.sqrt.f32(float %t291)
  ; let dist13 = %t366
  %t368 = call float @llvm.sqrt.f32(float %t307)
  ; let dist14 = %t368
  %t370 = call float @llvm.sqrt.f32(float %t323)
  ; let dist23 = %t370
  %t372 = call float @llvm.sqrt.f32(float %t339)
  ; let dist24 = %t372
  %t374 = call float @llvm.sqrt.f32(float %t355)
  ; let dist34 = %t374
  %il378 = load float, float* @dt, align 4
%t382 = fmul fast float %t211, %t356
%t383 = fdiv fast float %il378, %t382
  ; let mag01 = %t383
  %il386 = load float, float* @dt, align 4
%t390 = fmul fast float %t227, %t358
%t391 = fdiv fast float %il386, %t390
  ; let mag02 = %t391
  %il394 = load float, float* @dt, align 4
%t398 = fmul fast float %t243, %t360
%t399 = fdiv fast float %il394, %t398
  ; let mag03 = %t399
  %il402 = load float, float* @dt, align 4
%t406 = fmul fast float %t259, %t362
%t407 = fdiv fast float %il402, %t406
  ; let mag04 = %t407
  %il410 = load float, float* @dt, align 4
%t414 = fmul fast float %t275, %t364
%t415 = fdiv fast float %il410, %t414
  ; let mag12 = %t415
  %il418 = load float, float* @dt, align 4
%t422 = fmul fast float %t291, %t366
%t423 = fdiv fast float %il418, %t422
  ; let mag13 = %t423
  %il426 = load float, float* @dt, align 4
%t430 = fmul fast float %t307, %t368
%t431 = fdiv fast float %il426, %t430
  ; let mag14 = %t431
  %il434 = load float, float* @dt, align 4
%t438 = fmul fast float %t323, %t370
%t439 = fdiv fast float %il434, %t438
  ; let mag23 = %t439
  %il442 = load float, float* @dt, align 4
%t446 = fmul fast float %t339, %t372
%t447 = fdiv fast float %il442, %t446
  ; let mag24 = %t447
  %il450 = load float, float* @dt, align 4
%t454 = fmul fast float %t355, %t374
%t455 = fdiv fast float %il450, %t454
  ; let mag34 = %t455
  %fdp461 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t460 = load float, ptr %fdp461, align 4
  %il466 = load float, float* @m1, align 4
%t467 = fmul fast float %t26, %il466
%t469 = fmul fast float %t467, %t383
%t470 = fsub fast float %t460, %t469
  %il475 = load float, float* @m2, align 4
%t476 = fmul fast float %t44, %il475
%t478 = fmul fast float %t476, %t391
%t479 = fsub fast float %t470, %t478
  %il484 = load float, float* @m3, align 4
%t485 = fmul fast float %t62, %il484
%t487 = fmul fast float %t485, %t399
%t488 = fsub fast float %t479, %t487
  %il493 = load float, float* @m4, align 4
%t494 = fmul fast float %t80, %il493
%t496 = fmul fast float %t494, %t407
%t497 = fsub fast float %t488, %t496
  ; let nvz0 = %t497
  %fdp503 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t502 = load float, ptr %fdp503, align 4
  %il508 = load float, float* @m1, align 4
%t509 = fmul fast float %t20, %il508
%t511 = fmul fast float %t509, %t383
%t512 = fsub fast float %t502, %t511
  %il517 = load float, float* @m2, align 4
%t518 = fmul fast float %t38, %il517
%t520 = fmul fast float %t518, %t391
%t521 = fsub fast float %t512, %t520
  %il526 = load float, float* @m3, align 4
%t527 = fmul fast float %t56, %il526
%t529 = fmul fast float %t527, %t399
%t530 = fsub fast float %t521, %t529
  %il535 = load float, float* @m4, align 4
%t536 = fmul fast float %t74, %il535
%t538 = fmul fast float %t536, %t407
%t539 = fsub fast float %t530, %t538
  ; let nvy0 = %t539
  %fdp545 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t544 = load float, ptr %fdp545, align 4
  %il550 = load float, float* @m1, align 4
%t551 = fmul fast float %t14, %il550
%t553 = fmul fast float %t551, %t383
%t554 = fsub fast float %t544, %t553
  %il559 = load float, float* @m2, align 4
%t560 = fmul fast float %t32, %il559
%t562 = fmul fast float %t560, %t391
%t563 = fsub fast float %t554, %t562
  %il568 = load float, float* @m3, align 4
%t569 = fmul fast float %t50, %il568
%t571 = fmul fast float %t569, %t399
%t572 = fsub fast float %t563, %t571
  %il577 = load float, float* @m4, align 4
%t578 = fmul fast float %t68, %il577
%t580 = fmul fast float %t578, %t407
%t581 = fsub fast float %t572, %t580
  ; let nvx0 = %t581
  %fdp587 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t586 = load float, ptr %fdp587, align 4
  %il592 = load float, float* @m0, align 4
%t593 = fmul fast float %t20, %il592
%t595 = fmul fast float %t593, %t383
%t596 = fadd fast float %t586, %t595
  %il601 = load float, float* @m2, align 4
%t602 = fmul fast float %t92, %il601
%t604 = fmul fast float %t602, %t415
%t605 = fsub fast float %t596, %t604
  %il610 = load float, float* @m3, align 4
%t611 = fmul fast float %t110, %il610
%t613 = fmul fast float %t611, %t423
%t614 = fsub fast float %t605, %t613
  %il619 = load float, float* @m4, align 4
%t620 = fmul fast float %t128, %il619
%t622 = fmul fast float %t620, %t431
%t623 = fsub fast float %t614, %t622
  ; let nvy1 = %t623
  %fdp629 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t628 = load float, ptr %fdp629, align 4
  %il634 = load float, float* @m0, align 4
%t635 = fmul fast float %t14, %il634
%t637 = fmul fast float %t635, %t383
%t638 = fadd fast float %t628, %t637
  %il643 = load float, float* @m2, align 4
%t644 = fmul fast float %t86, %il643
%t646 = fmul fast float %t644, %t415
%t647 = fsub fast float %t638, %t646
  %il652 = load float, float* @m3, align 4
%t653 = fmul fast float %t104, %il652
%t655 = fmul fast float %t653, %t423
%t656 = fsub fast float %t647, %t655
  %il661 = load float, float* @m4, align 4
%t662 = fmul fast float %t122, %il661
%t664 = fmul fast float %t662, %t431
%t665 = fsub fast float %t656, %t664
  ; let nvx1 = %t665
  %fdp671 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t670 = load float, ptr %fdp671, align 4
  %il676 = load float, float* @m0, align 4
%t677 = fmul fast float %t26, %il676
%t679 = fmul fast float %t677, %t383
%t680 = fadd fast float %t670, %t679
  %il685 = load float, float* @m2, align 4
%t686 = fmul fast float %t98, %il685
%t688 = fmul fast float %t686, %t415
%t689 = fsub fast float %t680, %t688
  %il694 = load float, float* @m3, align 4
%t695 = fmul fast float %t116, %il694
%t697 = fmul fast float %t695, %t423
%t698 = fsub fast float %t689, %t697
  %il703 = load float, float* @m4, align 4
%t704 = fmul fast float %t134, %il703
%t706 = fmul fast float %t704, %t431
%t707 = fsub fast float %t698, %t706
  ; let nvz1 = %t707
  %fdp713 = getelementptr inbounds %State, ptr %state, i32 0, i32 19
  %t712 = load float, ptr %fdp713, align 4
  %il718 = load float, float* @m0, align 4
%t719 = fmul fast float %t44, %il718
%t721 = fmul fast float %t719, %t391
%t722 = fadd fast float %t712, %t721
  %il727 = load float, float* @m1, align 4
%t728 = fmul fast float %t98, %il727
%t730 = fmul fast float %t728, %t415
%t731 = fadd fast float %t722, %t730
  %il736 = load float, float* @m3, align 4
%t737 = fmul fast float %t152, %il736
%t739 = fmul fast float %t737, %t439
%t740 = fsub fast float %t731, %t739
  %il745 = load float, float* @m4, align 4
%t746 = fmul fast float %t170, %il745
%t748 = fmul fast float %t746, %t447
%t749 = fsub fast float %t740, %t748
  ; let nvz2 = %t749
  %fdp755 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
  %t754 = load float, ptr %fdp755, align 4
  %il760 = load float, float* @m0, align 4
%t761 = fmul fast float %t32, %il760
%t763 = fmul fast float %t761, %t391
%t764 = fadd fast float %t754, %t763
  %il769 = load float, float* @m1, align 4
%t770 = fmul fast float %t86, %il769
%t772 = fmul fast float %t770, %t415
%t773 = fadd fast float %t764, %t772
  %il778 = load float, float* @m3, align 4
%t779 = fmul fast float %t140, %il778
%t781 = fmul fast float %t779, %t439
%t782 = fsub fast float %t773, %t781
  %il787 = load float, float* @m4, align 4
%t788 = fmul fast float %t158, %il787
%t790 = fmul fast float %t788, %t447
%t791 = fsub fast float %t782, %t790
  ; let nvx2 = %t791
  %fdp797 = getelementptr inbounds %State, ptr %state, i32 0, i32 18
  %t796 = load float, ptr %fdp797, align 4
  %il802 = load float, float* @m0, align 4
%t803 = fmul fast float %t38, %il802
%t805 = fmul fast float %t803, %t391
%t806 = fadd fast float %t796, %t805
  %il811 = load float, float* @m1, align 4
%t812 = fmul fast float %t92, %il811
%t814 = fmul fast float %t812, %t415
%t815 = fadd fast float %t806, %t814
  %il820 = load float, float* @m3, align 4
%t821 = fmul fast float %t146, %il820
%t823 = fmul fast float %t821, %t439
%t824 = fsub fast float %t815, %t823
  %il829 = load float, float* @m4, align 4
%t830 = fmul fast float %t164, %il829
%t832 = fmul fast float %t830, %t447
%t833 = fsub fast float %t824, %t832
  ; let nvy2 = %t833
  %fdp839 = getelementptr inbounds %State, ptr %state, i32 0, i32 23
  %t838 = load float, ptr %fdp839, align 4
  %il844 = load float, float* @m0, align 4
%t845 = fmul fast float %t50, %il844
%t847 = fmul fast float %t845, %t399
%t848 = fadd fast float %t838, %t847
  %il853 = load float, float* @m1, align 4
%t854 = fmul fast float %t104, %il853
%t856 = fmul fast float %t854, %t423
%t857 = fadd fast float %t848, %t856
  %il862 = load float, float* @m2, align 4
%t863 = fmul fast float %t140, %il862
%t865 = fmul fast float %t863, %t439
%t866 = fadd fast float %t857, %t865
  %il871 = load float, float* @m4, align 4
%t872 = fmul fast float %t176, %il871
%t874 = fmul fast float %t872, %t455
%t875 = fsub fast float %t866, %t874
  ; let nvx3 = %t875
  %fdp881 = getelementptr inbounds %State, ptr %state, i32 0, i32 24
  %t880 = load float, ptr %fdp881, align 4
  %il886 = load float, float* @m0, align 4
%t887 = fmul fast float %t56, %il886
%t889 = fmul fast float %t887, %t399
%t890 = fadd fast float %t880, %t889
  %il895 = load float, float* @m1, align 4
%t896 = fmul fast float %t110, %il895
%t898 = fmul fast float %t896, %t423
%t899 = fadd fast float %t890, %t898
  %il904 = load float, float* @m2, align 4
%t905 = fmul fast float %t146, %il904
%t907 = fmul fast float %t905, %t439
%t908 = fadd fast float %t899, %t907
  %il913 = load float, float* @m4, align 4
%t914 = fmul fast float %t182, %il913
%t916 = fmul fast float %t914, %t455
%t917 = fsub fast float %t908, %t916
  ; let nvy3 = %t917
  %fdp923 = getelementptr inbounds %State, ptr %state, i32 0, i32 25
  %t922 = load float, ptr %fdp923, align 4
  %il928 = load float, float* @m0, align 4
%t929 = fmul fast float %t62, %il928
%t931 = fmul fast float %t929, %t399
%t932 = fadd fast float %t922, %t931
  %il937 = load float, float* @m1, align 4
%t938 = fmul fast float %t116, %il937
%t940 = fmul fast float %t938, %t423
%t941 = fadd fast float %t932, %t940
  %il946 = load float, float* @m2, align 4
%t947 = fmul fast float %t152, %il946
%t949 = fmul fast float %t947, %t439
%t950 = fadd fast float %t941, %t949
  %il955 = load float, float* @m4, align 4
%t956 = fmul fast float %t188, %il955
%t958 = fmul fast float %t956, %t455
%t959 = fsub fast float %t950, %t958
  ; let nvz3 = %t959
  %fdp965 = getelementptr inbounds %State, ptr %state, i32 0, i32 29
  %t964 = load float, ptr %fdp965, align 4
  %il970 = load float, float* @m0, align 4
%t971 = fmul fast float %t68, %il970
%t973 = fmul fast float %t971, %t407
%t974 = fadd fast float %t964, %t973
  %il979 = load float, float* @m1, align 4
%t980 = fmul fast float %t122, %il979
%t982 = fmul fast float %t980, %t431
%t983 = fadd fast float %t974, %t982
  %il988 = load float, float* @m2, align 4
%t989 = fmul fast float %t158, %il988
%t991 = fmul fast float %t989, %t447
%t992 = fadd fast float %t983, %t991
  %il997 = load float, float* @m3, align 4
%t998 = fmul fast float %t176, %il997
%t1000 = fmul fast float %t998, %t455
%t1001 = fadd fast float %t992, %t1000
  ; let nvx4 = %t1001
  %fdp1007 = getelementptr inbounds %State, ptr %state, i32 0, i32 30
  %t1006 = load float, ptr %fdp1007, align 4
  %il1012 = load float, float* @m0, align 4
%t1013 = fmul fast float %t74, %il1012
%t1015 = fmul fast float %t1013, %t407
%t1016 = fadd fast float %t1006, %t1015
  %il1021 = load float, float* @m1, align 4
%t1022 = fmul fast float %t128, %il1021
%t1024 = fmul fast float %t1022, %t431
%t1025 = fadd fast float %t1016, %t1024
  %il1030 = load float, float* @m2, align 4
%t1031 = fmul fast float %t164, %il1030
%t1033 = fmul fast float %t1031, %t447
%t1034 = fadd fast float %t1025, %t1033
  %il1039 = load float, float* @m3, align 4
%t1040 = fmul fast float %t182, %il1039
%t1042 = fmul fast float %t1040, %t455
%t1043 = fadd fast float %t1034, %t1042
  ; let nvy4 = %t1043
  %fdp1049 = getelementptr inbounds %State, ptr %state, i32 0, i32 31
  %t1048 = load float, ptr %fdp1049, align 4
  %il1054 = load float, float* @m0, align 4
%t1055 = fmul fast float %t80, %il1054
%t1057 = fmul fast float %t1055, %t407
%t1058 = fadd fast float %t1048, %t1057
  %il1063 = load float, float* @m1, align 4
%t1064 = fmul fast float %t134, %il1063
%t1066 = fmul fast float %t1064, %t431
%t1067 = fadd fast float %t1058, %t1066
  %il1072 = load float, float* @m2, align 4
%t1073 = fmul fast float %t170, %il1072
%t1075 = fmul fast float %t1073, %t447
%t1076 = fadd fast float %t1067, %t1075
  %il1081 = load float, float* @m3, align 4
%t1082 = fmul fast float %t188, %il1081
%t1084 = fmul fast float %t1082, %t455
%t1085 = fadd fast float %t1076, %t1084
  ; let nvz4 = %t1085
  %fdp1088 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1087 = load float, ptr %fdp1088, align 4
  %il1091 = load float, float* @dt, align 4
%t1093 = fmul fast float %il1091, %t497
%t1094 = fadd fast float %t1087, %t1093
  %ap_1095 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store float %t1094, ptr %ap_1095, align 4, !tbaa !3
  %ap_1097 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store float %t497, ptr %ap_1097, align 4, !tbaa !3
  %ap_1099 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store float %t539, ptr %ap_1099, align 4, !tbaa !3
  %fdp1102 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1101 = load float, ptr %fdp1102, align 4
  %il1105 = load float, float* @dt, align 4
%t1107 = fmul fast float %il1105, %t539
%t1108 = fadd fast float %t1101, %t1107
  %ap_1109 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store float %t1108, ptr %ap_1109, align 4, !tbaa !3
  %fdp1112 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1111 = load float, ptr %fdp1112, align 4
  %il1115 = load float, float* @dt, align 4
%t1117 = fmul fast float %il1115, %t581
%t1118 = fadd fast float %t1111, %t1117
  %ap_1119 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store float %t1118, ptr %ap_1119, align 4, !tbaa !3
  %ap_1121 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store float %t581, ptr %ap_1121, align 4, !tbaa !3
  %ap_1123 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store float %t623, ptr %ap_1123, align 4, !tbaa !3
  %fdp1126 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1125 = load float, ptr %fdp1126, align 4
  %il1129 = load float, float* @dt, align 4
%t1131 = fmul fast float %il1129, %t623
%t1132 = fadd fast float %t1125, %t1131
  %ap_1133 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store float %t1132, ptr %ap_1133, align 4, !tbaa !3
  %ap_1135 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  store float %t665, ptr %ap_1135, align 4, !tbaa !3
  %fdp1138 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1137 = load float, ptr %fdp1138, align 4
  %il1141 = load float, float* @dt, align 4
%t1143 = fmul fast float %il1141, %t665
%t1144 = fadd fast float %t1137, %t1143
  %ap_1145 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store float %t1144, ptr %ap_1145, align 4, !tbaa !3
  %fdp1148 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1147 = load float, ptr %fdp1148, align 4
  %il1151 = load float, float* @dt, align 4
%t1153 = fmul fast float %il1151, %t707
%t1154 = fadd fast float %t1147, %t1153
  %ap_1155 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store float %t1154, ptr %ap_1155, align 4, !tbaa !3
  %ap_1157 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  store float %t707, ptr %ap_1157, align 4, !tbaa !3
  %fdp1160 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1159 = load float, ptr %fdp1160, align 4
  %il1163 = load float, float* @dt, align 4
%t1165 = fmul fast float %il1163, %t749
%t1166 = fadd fast float %t1159, %t1165
  %ap_1167 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  store float %t1166, ptr %ap_1167, align 4, !tbaa !3
  %ap_1169 = getelementptr inbounds %State, ptr %state, i32 0, i32 19
  store float %t749, ptr %ap_1169, align 4, !tbaa !3
  %fdp1172 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1171 = load float, ptr %fdp1172, align 4
  %il1175 = load float, float* @dt, align 4
%t1177 = fmul fast float %il1175, %t791
%t1178 = fadd fast float %t1171, %t1177
  %ap_1179 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  store float %t1178, ptr %ap_1179, align 4, !tbaa !3
  %ap_1181 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
  store float %t791, ptr %ap_1181, align 4, !tbaa !3
  %ap_1183 = getelementptr inbounds %State, ptr %state, i32 0, i32 18
  store float %t833, ptr %ap_1183, align 4, !tbaa !3
  %fdp1186 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1185 = load float, ptr %fdp1186, align 4
  %il1189 = load float, float* @dt, align 4
%t1191 = fmul fast float %il1189, %t833
%t1192 = fadd fast float %t1185, %t1191
  %ap_1193 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  store float %t1192, ptr %ap_1193, align 4, !tbaa !3
  %fdp1196 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1195 = load float, ptr %fdp1196, align 4
  %il1199 = load float, float* @dt, align 4
%t1201 = fmul fast float %il1199, %t875
%t1202 = fadd fast float %t1195, %t1201
  %ap_1203 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  store float %t1202, ptr %ap_1203, align 4, !tbaa !3
  %ap_1205 = getelementptr inbounds %State, ptr %state, i32 0, i32 23
  store float %t875, ptr %ap_1205, align 4, !tbaa !3
  %ap_1207 = getelementptr inbounds %State, ptr %state, i32 0, i32 24
  store float %t917, ptr %ap_1207, align 4, !tbaa !3
  %fdp1210 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1209 = load float, ptr %fdp1210, align 4
  %il1213 = load float, float* @dt, align 4
%t1215 = fmul fast float %il1213, %t917
%t1216 = fadd fast float %t1209, %t1215
  %ap_1217 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  store float %t1216, ptr %ap_1217, align 4, !tbaa !3
  %ap_1219 = getelementptr inbounds %State, ptr %state, i32 0, i32 25
  store float %t959, ptr %ap_1219, align 4, !tbaa !3
  %fdp1222 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1221 = load float, ptr %fdp1222, align 4
  %il1225 = load float, float* @dt, align 4
%t1227 = fmul fast float %il1225, %t959
%t1228 = fadd fast float %t1221, %t1227
  %ap_1229 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  store float %t1228, ptr %ap_1229, align 4, !tbaa !3
  %fdp1232 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1231 = load float, ptr %fdp1232, align 4
  %il1235 = load float, float* @dt, align 4
%t1237 = fmul fast float %il1235, %t1001
%t1238 = fadd fast float %t1231, %t1237
  %ap_1239 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  store float %t1238, ptr %ap_1239, align 4, !tbaa !3
  %ap_1241 = getelementptr inbounds %State, ptr %state, i32 0, i32 29
  store float %t1001, ptr %ap_1241, align 4, !tbaa !3
  %fdp1244 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1243 = load float, ptr %fdp1244, align 4
  %il1247 = load float, float* @dt, align 4
%t1249 = fmul fast float %il1247, %t1043
%t1250 = fadd fast float %t1243, %t1249
  %ap_1251 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  store float %t1250, ptr %ap_1251, align 4, !tbaa !3
  %ap_1253 = getelementptr inbounds %State, ptr %state, i32 0, i32 30
  store float %t1043, ptr %ap_1253, align 4, !tbaa !3
  %fdp1256 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1255 = load float, ptr %fdp1256, align 4
  %il1259 = load float, float* @dt, align 4
%t1261 = fmul fast float %il1259, %t1085
%t1262 = fadd fast float %t1255, %t1261
  %ap_1263 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  store float %t1262, ptr %ap_1263, align 4, !tbaa !3
  %ap_1265 = getelementptr inbounds %State, ptr %state, i32 0, i32 31
  store float %t1085, ptr %ap_1265, align 4, !tbaa !3
  %fdp1268 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t1267 = load i64, i64* %fdp1268, align 8
  %fdp1270 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t1269 = load i64, i64* %fdp1270, align 8
%c1271 = icmp eq i64 %t1267, %t1269
  br i1 %c1271, label %g1272_t, label %g1272_e
  g1272_t:
  %fdp1279 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1278 = load float, ptr %fdp1279, align 4
  %fdp1281 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1280 = load float, ptr %fdp1281, align 4
%t1282 = fsub fast float %t1278, %t1280
  %fdp1285 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1284 = load float, ptr %fdp1285, align 4
  %fdp1287 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1286 = load float, ptr %fdp1287, align 4
%t1288 = fsub fast float %t1284, %t1286
%t1289 = fmul fast float %t1282, %t1288
  %fdp1293 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1292 = load float, ptr %fdp1293, align 4
  %fdp1295 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1294 = load float, ptr %fdp1295, align 4
%t1296 = fsub fast float %t1292, %t1294
  %fdp1299 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1298 = load float, ptr %fdp1299, align 4
  %fdp1301 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1300 = load float, ptr %fdp1301, align 4
%t1302 = fsub fast float %t1298, %t1300
%t1303 = fmul fast float %t1296, %t1302
%t1304 = fadd fast float %t1289, %t1303
  %fdp1308 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1307 = load float, ptr %fdp1308, align 4
  %fdp1310 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1309 = load float, ptr %fdp1310, align 4
%t1311 = fsub fast float %t1307, %t1309
  %fdp1314 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1313 = load float, ptr %fdp1314, align 4
  %fdp1316 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1315 = load float, ptr %fdp1316, align 4
%t1317 = fsub fast float %t1313, %t1315
%t1318 = fmul fast float %t1311, %t1317
%t1319 = fadd fast float %t1304, %t1318
    %t1273 = call float @llvm.sqrt.f32(float %t1319)
    ; let edist01 = %t1273
  %fdp1326 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1325 = load float, ptr %fdp1326, align 4
  %fdp1328 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1327 = load float, ptr %fdp1328, align 4
%t1329 = fsub fast float %t1325, %t1327
  %fdp1332 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1331 = load float, ptr %fdp1332, align 4
  %fdp1334 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1333 = load float, ptr %fdp1334, align 4
%t1335 = fsub fast float %t1331, %t1333
%t1336 = fmul fast float %t1329, %t1335
  %fdp1340 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1339 = load float, ptr %fdp1340, align 4
  %fdp1342 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1341 = load float, ptr %fdp1342, align 4
%t1343 = fsub fast float %t1339, %t1341
  %fdp1346 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1345 = load float, ptr %fdp1346, align 4
  %fdp1348 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1347 = load float, ptr %fdp1348, align 4
%t1349 = fsub fast float %t1345, %t1347
%t1350 = fmul fast float %t1343, %t1349
%t1351 = fadd fast float %t1336, %t1350
  %fdp1355 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1354 = load float, ptr %fdp1355, align 4
  %fdp1357 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1356 = load float, ptr %fdp1357, align 4
%t1358 = fsub fast float %t1354, %t1356
  %fdp1361 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1360 = load float, ptr %fdp1361, align 4
  %fdp1363 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1362 = load float, ptr %fdp1363, align 4
%t1364 = fsub fast float %t1360, %t1362
%t1365 = fmul fast float %t1358, %t1364
%t1366 = fadd fast float %t1351, %t1365
    %t1320 = call float @llvm.sqrt.f32(float %t1366)
    ; let edist02 = %t1320
  %fdp1373 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1372 = load float, ptr %fdp1373, align 4
  %fdp1375 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1374 = load float, ptr %fdp1375, align 4
%t1376 = fsub fast float %t1372, %t1374
  %fdp1379 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1378 = load float, ptr %fdp1379, align 4
  %fdp1381 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1380 = load float, ptr %fdp1381, align 4
%t1382 = fsub fast float %t1378, %t1380
%t1383 = fmul fast float %t1376, %t1382
  %fdp1387 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1386 = load float, ptr %fdp1387, align 4
  %fdp1389 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1388 = load float, ptr %fdp1389, align 4
%t1390 = fsub fast float %t1386, %t1388
  %fdp1393 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1392 = load float, ptr %fdp1393, align 4
  %fdp1395 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1394 = load float, ptr %fdp1395, align 4
%t1396 = fsub fast float %t1392, %t1394
%t1397 = fmul fast float %t1390, %t1396
%t1398 = fadd fast float %t1383, %t1397
  %fdp1402 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1401 = load float, ptr %fdp1402, align 4
  %fdp1404 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1403 = load float, ptr %fdp1404, align 4
%t1405 = fsub fast float %t1401, %t1403
  %fdp1408 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1407 = load float, ptr %fdp1408, align 4
  %fdp1410 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1409 = load float, ptr %fdp1410, align 4
%t1411 = fsub fast float %t1407, %t1409
%t1412 = fmul fast float %t1405, %t1411
%t1413 = fadd fast float %t1398, %t1412
    %t1367 = call float @llvm.sqrt.f32(float %t1413)
    ; let edist03 = %t1367
  %fdp1420 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1419 = load float, ptr %fdp1420, align 4
  %fdp1422 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1421 = load float, ptr %fdp1422, align 4
%t1423 = fsub fast float %t1419, %t1421
  %fdp1426 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t1425 = load float, ptr %fdp1426, align 4
  %fdp1428 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1427 = load float, ptr %fdp1428, align 4
%t1429 = fsub fast float %t1425, %t1427
%t1430 = fmul fast float %t1423, %t1429
  %fdp1434 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1433 = load float, ptr %fdp1434, align 4
  %fdp1436 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1435 = load float, ptr %fdp1436, align 4
%t1437 = fsub fast float %t1433, %t1435
  %fdp1440 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t1439 = load float, ptr %fdp1440, align 4
  %fdp1442 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1441 = load float, ptr %fdp1442, align 4
%t1443 = fsub fast float %t1439, %t1441
%t1444 = fmul fast float %t1437, %t1443
%t1445 = fadd fast float %t1430, %t1444
  %fdp1449 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1448 = load float, ptr %fdp1449, align 4
  %fdp1451 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1450 = load float, ptr %fdp1451, align 4
%t1452 = fsub fast float %t1448, %t1450
  %fdp1455 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t1454 = load float, ptr %fdp1455, align 4
  %fdp1457 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1456 = load float, ptr %fdp1457, align 4
%t1458 = fsub fast float %t1454, %t1456
%t1459 = fmul fast float %t1452, %t1458
%t1460 = fadd fast float %t1445, %t1459
    %t1414 = call float @llvm.sqrt.f32(float %t1460)
    ; let edist04 = %t1414
  %fdp1467 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1466 = load float, ptr %fdp1467, align 4
  %fdp1469 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1468 = load float, ptr %fdp1469, align 4
%t1470 = fsub fast float %t1466, %t1468
  %fdp1473 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1472 = load float, ptr %fdp1473, align 4
  %fdp1475 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1474 = load float, ptr %fdp1475, align 4
%t1476 = fsub fast float %t1472, %t1474
%t1477 = fmul fast float %t1470, %t1476
  %fdp1481 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1480 = load float, ptr %fdp1481, align 4
  %fdp1483 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1482 = load float, ptr %fdp1483, align 4
%t1484 = fsub fast float %t1480, %t1482
  %fdp1487 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1486 = load float, ptr %fdp1487, align 4
  %fdp1489 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1488 = load float, ptr %fdp1489, align 4
%t1490 = fsub fast float %t1486, %t1488
%t1491 = fmul fast float %t1484, %t1490
%t1492 = fadd fast float %t1477, %t1491
  %fdp1496 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1495 = load float, ptr %fdp1496, align 4
  %fdp1498 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1497 = load float, ptr %fdp1498, align 4
%t1499 = fsub fast float %t1495, %t1497
  %fdp1502 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1501 = load float, ptr %fdp1502, align 4
  %fdp1504 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1503 = load float, ptr %fdp1504, align 4
%t1505 = fsub fast float %t1501, %t1503
%t1506 = fmul fast float %t1499, %t1505
%t1507 = fadd fast float %t1492, %t1506
    %t1461 = call float @llvm.sqrt.f32(float %t1507)
    ; let edist12 = %t1461
  %fdp1514 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1513 = load float, ptr %fdp1514, align 4
  %fdp1516 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1515 = load float, ptr %fdp1516, align 4
%t1517 = fsub fast float %t1513, %t1515
  %fdp1520 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1519 = load float, ptr %fdp1520, align 4
  %fdp1522 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1521 = load float, ptr %fdp1522, align 4
%t1523 = fsub fast float %t1519, %t1521
%t1524 = fmul fast float %t1517, %t1523
  %fdp1528 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1527 = load float, ptr %fdp1528, align 4
  %fdp1530 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1529 = load float, ptr %fdp1530, align 4
%t1531 = fsub fast float %t1527, %t1529
  %fdp1534 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1533 = load float, ptr %fdp1534, align 4
  %fdp1536 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1535 = load float, ptr %fdp1536, align 4
%t1537 = fsub fast float %t1533, %t1535
%t1538 = fmul fast float %t1531, %t1537
%t1539 = fadd fast float %t1524, %t1538
  %fdp1543 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1542 = load float, ptr %fdp1543, align 4
  %fdp1545 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1544 = load float, ptr %fdp1545, align 4
%t1546 = fsub fast float %t1542, %t1544
  %fdp1549 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1548 = load float, ptr %fdp1549, align 4
  %fdp1551 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1550 = load float, ptr %fdp1551, align 4
%t1552 = fsub fast float %t1548, %t1550
%t1553 = fmul fast float %t1546, %t1552
%t1554 = fadd fast float %t1539, %t1553
    %t1508 = call float @llvm.sqrt.f32(float %t1554)
    ; let edist13 = %t1508
  %fdp1561 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1560 = load float, ptr %fdp1561, align 4
  %fdp1563 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1562 = load float, ptr %fdp1563, align 4
%t1564 = fsub fast float %t1560, %t1562
  %fdp1567 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t1566 = load float, ptr %fdp1567, align 4
  %fdp1569 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1568 = load float, ptr %fdp1569, align 4
%t1570 = fsub fast float %t1566, %t1568
%t1571 = fmul fast float %t1564, %t1570
  %fdp1575 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1574 = load float, ptr %fdp1575, align 4
  %fdp1577 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1576 = load float, ptr %fdp1577, align 4
%t1578 = fsub fast float %t1574, %t1576
  %fdp1581 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t1580 = load float, ptr %fdp1581, align 4
  %fdp1583 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1582 = load float, ptr %fdp1583, align 4
%t1584 = fsub fast float %t1580, %t1582
%t1585 = fmul fast float %t1578, %t1584
%t1586 = fadd fast float %t1571, %t1585
  %fdp1590 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1589 = load float, ptr %fdp1590, align 4
  %fdp1592 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1591 = load float, ptr %fdp1592, align 4
%t1593 = fsub fast float %t1589, %t1591
  %fdp1596 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t1595 = load float, ptr %fdp1596, align 4
  %fdp1598 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1597 = load float, ptr %fdp1598, align 4
%t1599 = fsub fast float %t1595, %t1597
%t1600 = fmul fast float %t1593, %t1599
%t1601 = fadd fast float %t1586, %t1600
    %t1555 = call float @llvm.sqrt.f32(float %t1601)
    ; let edist14 = %t1555
  %fdp1608 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1607 = load float, ptr %fdp1608, align 4
  %fdp1610 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1609 = load float, ptr %fdp1610, align 4
%t1611 = fsub fast float %t1607, %t1609
  %fdp1614 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1613 = load float, ptr %fdp1614, align 4
  %fdp1616 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1615 = load float, ptr %fdp1616, align 4
%t1617 = fsub fast float %t1613, %t1615
%t1618 = fmul fast float %t1611, %t1617
  %fdp1622 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1621 = load float, ptr %fdp1622, align 4
  %fdp1624 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1623 = load float, ptr %fdp1624, align 4
%t1625 = fsub fast float %t1621, %t1623
  %fdp1628 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1627 = load float, ptr %fdp1628, align 4
  %fdp1630 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1629 = load float, ptr %fdp1630, align 4
%t1631 = fsub fast float %t1627, %t1629
%t1632 = fmul fast float %t1625, %t1631
%t1633 = fadd fast float %t1618, %t1632
  %fdp1637 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1636 = load float, ptr %fdp1637, align 4
  %fdp1639 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1638 = load float, ptr %fdp1639, align 4
%t1640 = fsub fast float %t1636, %t1638
  %fdp1643 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1642 = load float, ptr %fdp1643, align 4
  %fdp1645 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1644 = load float, ptr %fdp1645, align 4
%t1646 = fsub fast float %t1642, %t1644
%t1647 = fmul fast float %t1640, %t1646
%t1648 = fadd fast float %t1633, %t1647
    %t1602 = call float @llvm.sqrt.f32(float %t1648)
    ; let edist23 = %t1602
  %fdp1655 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1654 = load float, ptr %fdp1655, align 4
  %fdp1657 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1656 = load float, ptr %fdp1657, align 4
%t1658 = fsub fast float %t1654, %t1656
  %fdp1661 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t1660 = load float, ptr %fdp1661, align 4
  %fdp1663 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1662 = load float, ptr %fdp1663, align 4
%t1664 = fsub fast float %t1660, %t1662
%t1665 = fmul fast float %t1658, %t1664
  %fdp1669 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1668 = load float, ptr %fdp1669, align 4
  %fdp1671 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1670 = load float, ptr %fdp1671, align 4
%t1672 = fsub fast float %t1668, %t1670
  %fdp1675 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t1674 = load float, ptr %fdp1675, align 4
  %fdp1677 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1676 = load float, ptr %fdp1677, align 4
%t1678 = fsub fast float %t1674, %t1676
%t1679 = fmul fast float %t1672, %t1678
%t1680 = fadd fast float %t1665, %t1679
  %fdp1684 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1683 = load float, ptr %fdp1684, align 4
  %fdp1686 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1685 = load float, ptr %fdp1686, align 4
%t1687 = fsub fast float %t1683, %t1685
  %fdp1690 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t1689 = load float, ptr %fdp1690, align 4
  %fdp1692 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1691 = load float, ptr %fdp1692, align 4
%t1693 = fsub fast float %t1689, %t1691
%t1694 = fmul fast float %t1687, %t1693
%t1695 = fadd fast float %t1680, %t1694
    %t1649 = call float @llvm.sqrt.f32(float %t1695)
    ; let edist24 = %t1649
  %fdp1702 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1701 = load float, ptr %fdp1702, align 4
  %fdp1704 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1703 = load float, ptr %fdp1704, align 4
%t1705 = fsub fast float %t1701, %t1703
  %fdp1708 = getelementptr inbounds %State, ptr %state, i32 0, i32 20
  %t1707 = load float, ptr %fdp1708, align 4
  %fdp1710 = getelementptr inbounds %State, ptr %state, i32 0, i32 26
  %t1709 = load float, ptr %fdp1710, align 4
%t1711 = fsub fast float %t1707, %t1709
%t1712 = fmul fast float %t1705, %t1711
  %fdp1716 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1715 = load float, ptr %fdp1716, align 4
  %fdp1718 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1717 = load float, ptr %fdp1718, align 4
%t1719 = fsub fast float %t1715, %t1717
  %fdp1722 = getelementptr inbounds %State, ptr %state, i32 0, i32 21
  %t1721 = load float, ptr %fdp1722, align 4
  %fdp1724 = getelementptr inbounds %State, ptr %state, i32 0, i32 27
  %t1723 = load float, ptr %fdp1724, align 4
%t1725 = fsub fast float %t1721, %t1723
%t1726 = fmul fast float %t1719, %t1725
%t1727 = fadd fast float %t1712, %t1726
  %fdp1731 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1730 = load float, ptr %fdp1731, align 4
  %fdp1733 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1732 = load float, ptr %fdp1733, align 4
%t1734 = fsub fast float %t1730, %t1732
  %fdp1737 = getelementptr inbounds %State, ptr %state, i32 0, i32 22
  %t1736 = load float, ptr %fdp1737, align 4
  %fdp1739 = getelementptr inbounds %State, ptr %state, i32 0, i32 28
  %t1738 = load float, ptr %fdp1739, align 4
%t1740 = fsub fast float %t1736, %t1738
%t1741 = fmul fast float %t1734, %t1740
%t1742 = fadd fast float %t1727, %t1741
    %t1696 = call float @llvm.sqrt.f32(float %t1742)
    ; let edist34 = %t1696
  %il1746 = load float, float* @m0, align 4
  %il1748 = load float, float* @m1, align 4
%t1749 = fmul fast float %il1746, %il1748
%t1751 = fdiv fast float %t1749, %t1273
    ; let e01 = %t1751
  %il1755 = load float, float* @m0, align 4
  %il1757 = load float, float* @m2, align 4
%t1758 = fmul fast float %il1755, %il1757
%t1760 = fdiv fast float %t1758, %t1320
    ; let e02 = %t1760
  %il1764 = load float, float* @m0, align 4
  %il1766 = load float, float* @m3, align 4
%t1767 = fmul fast float %il1764, %il1766
%t1769 = fdiv fast float %t1767, %t1367
    ; let e03 = %t1769
  %il1773 = load float, float* @m0, align 4
  %il1775 = load float, float* @m4, align 4
%t1776 = fmul fast float %il1773, %il1775
%t1778 = fdiv fast float %t1776, %t1414
    ; let e04 = %t1778
  %il1782 = load float, float* @m1, align 4
  %il1784 = load float, float* @m2, align 4
%t1785 = fmul fast float %il1782, %il1784
%t1787 = fdiv fast float %t1785, %t1461
    ; let e12 = %t1787
  %il1791 = load float, float* @m1, align 4
  %il1793 = load float, float* @m3, align 4
%t1794 = fmul fast float %il1791, %il1793
%t1796 = fdiv fast float %t1794, %t1508
    ; let e13 = %t1796
  %il1800 = load float, float* @m1, align 4
  %il1802 = load float, float* @m4, align 4
%t1803 = fmul fast float %il1800, %il1802
%t1805 = fdiv fast float %t1803, %t1555
    ; let e14 = %t1805
  %il1809 = load float, float* @m2, align 4
  %il1811 = load float, float* @m3, align 4
%t1812 = fmul fast float %il1809, %il1811
%t1814 = fdiv fast float %t1812, %t1602
    ; let e23 = %t1814
  %il1818 = load float, float* @m2, align 4
  %il1820 = load float, float* @m4, align 4
%t1821 = fmul fast float %il1818, %il1820
%t1823 = fdiv fast float %t1821, %t1649
    ; let e24 = %t1823
  %il1827 = load float, float* @m3, align 4
  %il1829 = load float, float* @m4, align 4
%t1830 = fmul fast float %il1827, %il1829
%t1832 = fdiv fast float %t1830, %t1696
    ; let e34 = %t1832
%t1846 = fadd fast float %t1751, %t1760
%t1848 = fadd fast float %t1846, %t1769
%t1850 = fadd fast float %t1848, %t1778
%t1852 = fadd fast float %t1850, %t1787
%t1854 = fadd fast float %t1852, %t1796
%t1856 = fadd fast float %t1854, %t1805
%t1858 = fadd fast float %t1856, %t1814
%t1860 = fadd fast float %t1858, %t1823
%t1862 = fadd fast float %t1860, %t1832
  %t1863 = fneg float %t1862
    ; let ep = %t1863
%ff1868 = bitcast i32 1056964608 to float
  %il1870 = load float, float* @m0, align 4
%t1871 = fmul fast float %ff1868, %il1870
  %fdp1876 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t1875 = load float, ptr %fdp1876, align 4
  %fdp1878 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t1877 = load float, ptr %fdp1878, align 4
%t1879 = fmul fast float %t1875, %t1877
  %fdp1882 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t1881 = load float, ptr %fdp1882, align 4
  %fdp1884 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t1883 = load float, ptr %fdp1884, align 4
%t1885 = fmul fast float %t1881, %t1883
%t1886 = fadd fast float %t1879, %t1885
  %fdp1889 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t1888 = load float, ptr %fdp1889, align 4
  %fdp1891 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t1890 = load float, ptr %fdp1891, align 4
%t1892 = fmul fast float %t1888, %t1890
%t1893 = fadd fast float %t1886, %t1892
%t1894 = fmul fast float %t1871, %t1893
    ; let ek0 = %t1894
%ff1899 = bitcast i32 1056964608 to float
  %il1901 = load float, float* @m1, align 4
%t1902 = fmul fast float %ff1899, %il1901
  %fdp1907 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t1906 = load float, ptr %fdp1907, align 4
  %fdp1909 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t1908 = load float, ptr %fdp1909, align 4
%t1910 = fmul fast float %t1906, %t1908
  %fdp1913 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t1912 = load float, ptr %fdp1913, align 4
  %fdp1915 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t1914 = load float, ptr %fdp1915, align 4
%t1916 = fmul fast float %t1912, %t1914
%t1917 = fadd fast float %t1910, %t1916
  %fdp1920 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t1919 = load float, ptr %fdp1920, align 4
  %fdp1922 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t1921 = load float, ptr %fdp1922, align 4
%t1923 = fmul fast float %t1919, %t1921
%t1924 = fadd fast float %t1917, %t1923
%t1925 = fmul fast float %t1902, %t1924
    ; let ek1 = %t1925
%ff1930 = bitcast i32 1056964608 to float
  %il1932 = load float, float* @m2, align 4
%t1933 = fmul fast float %ff1930, %il1932
  %fdp1938 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
  %t1937 = load float, ptr %fdp1938, align 4
  %fdp1940 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
  %t1939 = load float, ptr %fdp1940, align 4
%t1941 = fmul fast float %t1937, %t1939
  %fdp1944 = getelementptr inbounds %State, ptr %state, i32 0, i32 18
  %t1943 = load float, ptr %fdp1944, align 4
  %fdp1946 = getelementptr inbounds %State, ptr %state, i32 0, i32 18
  %t1945 = load float, ptr %fdp1946, align 4
%t1947 = fmul fast float %t1943, %t1945
%t1948 = fadd fast float %t1941, %t1947
  %fdp1951 = getelementptr inbounds %State, ptr %state, i32 0, i32 19
  %t1950 = load float, ptr %fdp1951, align 4
  %fdp1953 = getelementptr inbounds %State, ptr %state, i32 0, i32 19
  %t1952 = load float, ptr %fdp1953, align 4
%t1954 = fmul fast float %t1950, %t1952
%t1955 = fadd fast float %t1948, %t1954
%t1956 = fmul fast float %t1933, %t1955
    ; let ek2 = %t1956
%ff1961 = bitcast i32 1056964608 to float
  %il1963 = load float, float* @m3, align 4
%t1964 = fmul fast float %ff1961, %il1963
  %fdp1969 = getelementptr inbounds %State, ptr %state, i32 0, i32 23
  %t1968 = load float, ptr %fdp1969, align 4
  %fdp1971 = getelementptr inbounds %State, ptr %state, i32 0, i32 23
  %t1970 = load float, ptr %fdp1971, align 4
%t1972 = fmul fast float %t1968, %t1970
  %fdp1975 = getelementptr inbounds %State, ptr %state, i32 0, i32 24
  %t1974 = load float, ptr %fdp1975, align 4
  %fdp1977 = getelementptr inbounds %State, ptr %state, i32 0, i32 24
  %t1976 = load float, ptr %fdp1977, align 4
%t1978 = fmul fast float %t1974, %t1976
%t1979 = fadd fast float %t1972, %t1978
  %fdp1982 = getelementptr inbounds %State, ptr %state, i32 0, i32 25
  %t1981 = load float, ptr %fdp1982, align 4
  %fdp1984 = getelementptr inbounds %State, ptr %state, i32 0, i32 25
  %t1983 = load float, ptr %fdp1984, align 4
%t1985 = fmul fast float %t1981, %t1983
%t1986 = fadd fast float %t1979, %t1985
%t1987 = fmul fast float %t1964, %t1986
    ; let ek3 = %t1987
%ff1992 = bitcast i32 1056964608 to float
  %il1994 = load float, float* @m4, align 4
%t1995 = fmul fast float %ff1992, %il1994
  %fdp2000 = getelementptr inbounds %State, ptr %state, i32 0, i32 29
  %t1999 = load float, ptr %fdp2000, align 4
  %fdp2002 = getelementptr inbounds %State, ptr %state, i32 0, i32 29
  %t2001 = load float, ptr %fdp2002, align 4
%t2003 = fmul fast float %t1999, %t2001
  %fdp2006 = getelementptr inbounds %State, ptr %state, i32 0, i32 30
  %t2005 = load float, ptr %fdp2006, align 4
  %fdp2008 = getelementptr inbounds %State, ptr %state, i32 0, i32 30
  %t2007 = load float, ptr %fdp2008, align 4
%t2009 = fmul fast float %t2005, %t2007
%t2010 = fadd fast float %t2003, %t2009
  %fdp2013 = getelementptr inbounds %State, ptr %state, i32 0, i32 31
  %t2012 = load float, ptr %fdp2013, align 4
  %fdp2015 = getelementptr inbounds %State, ptr %state, i32 0, i32 31
  %t2014 = load float, ptr %fdp2015, align 4
%t2016 = fmul fast float %t2012, %t2014
%t2017 = fadd fast float %t2010, %t2016
%t2018 = fmul fast float %t1995, %t2017
    ; let ek4 = %t2018
%t2026 = fadd fast float %t1863, %t1894
%t2028 = fadd fast float %t2026, %t1925
%t2030 = fadd fast float %t2028, %t1956
%t2032 = fadd fast float %t2030, %t1987
%t2034 = fadd fast float %t2032, %t2018
    ; let energy = %t2034
    %pfd2037 = fpext float %t2034 to double
    %pso2038 = load volatile ptr, ptr @stdout
    %pff2039 = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0
    %ppf2040 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso2038, ptr %pff2039, double %pfd2037)
    %t2035 = zext i32 %ppf2040 to i64
    ret void
  g1272_e:
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
  %lv_474 = alloca <4 x float>, align 16
  %lv_475 = alloca <4 x float>, align 16
  %lv_476 = alloca float, align 4
  %lv_477 = alloca <4 x float>, align 16
  %lv_478 = alloca <4 x float>, align 16
  %lv_479 = alloca float, align 4
  %lv_480 = alloca float, align 4
  %lv_481 = alloca <4 x float>, align 16
  %lv_482 = alloca <4 x float>, align 16
  %lv_483 = alloca float, align 4
  %lv_484 = alloca float, align 4
  %lv_485 = alloca float, align 4
  %init_cnt_486 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %init_bound_487 = load i64, ptr %init_cnt_486, align 8, !tbaa !1
  %init_cnt_488 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %init_bx0_489 = load float, ptr %init_cnt_488, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_490 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
  %init_bx1_491 = load float, ptr %init_cnt_490, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_492 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 14
  %init_bx2_493 = load float, ptr %init_cnt_492, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_494 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 5
  %init_bx3_495 = load float, ptr %init_cnt_494, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_496 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 11
  %init_bx4_497 = load float, ptr %init_cnt_496, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_498 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %init_by0_499 = load float, ptr %init_cnt_498, align 4, !tbaa !3
  %init_cnt_500 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
  %init_by1_501 = load float, ptr %init_cnt_500, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_502 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 0
  %init_by2_503 = load float, ptr %init_cnt_502, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_504 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 6
  %init_by3_505 = load float, ptr %init_cnt_504, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_506 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 12
  %init_by4_507 = load float, ptr %init_cnt_506, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_508 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %init_bz0_509 = load float, ptr %init_cnt_508, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_510 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
  %init_bz1_511 = load float, ptr %init_cnt_510, align 4, !tbaa !3
  %init_cnt_512 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 1
  %init_bz2_513 = load float, ptr %init_cnt_512, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_514 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 7
  %init_bz3_515 = load float, ptr %init_cnt_514, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_516 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 13
  %init_bz4_517 = load float, ptr %init_cnt_516, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_518 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 2
  %init_cycle_count_519 = load i64, ptr %init_cnt_518, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_520 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %init_vx0_521 = load float, ptr %init_cnt_520, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_522 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
  %init_vx1_523 = load float, ptr %init_cnt_522, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_524 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 2
  %init_vx2_525 = load float, ptr %init_cnt_524, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_526 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 8
  %init_vx3_527 = load float, ptr %init_cnt_526, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_528 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 14
  %init_vx4_529 = load float, ptr %init_cnt_528, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_530 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %init_vy0_531 = load float, ptr %init_cnt_530, align 4, !tbaa !3
  %init_cnt_532 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
  %init_vy1_533 = load float, ptr %init_cnt_532, align 4, !tbaa !3
  %init_cnt_534 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 3
  %init_vy2_535 = load float, ptr %init_cnt_534, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_536 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 9
  %init_vy3_537 = load float, ptr %init_cnt_536, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_538 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 0
  %init_vy4_539 = load float, ptr %init_cnt_538, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_540 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
  %init_vz0_541 = load float, ptr %init_cnt_540, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_542 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
  %init_vz1_543 = load float, ptr %init_cnt_542, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_544 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 4
  %init_vz2_545 = load float, ptr %init_cnt_544, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_546 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 10
  %init_vz3_547 = load float, ptr %init_cnt_546, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_548 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 1
  %init_vz4_549 = load float, ptr %init_cnt_548, align 4, !tbaa !3, !invariant.load !{}
  %init_cnt_550 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
  %init_count_551 = load i64, ptr %init_cnt_550, align 8
  %iv553_phi_vz_v4553 = insertelement <4 x float> undef, float %init_vz0_541, i32 0
  %iv554_phi_vz_v4554 = insertelement <4 x float> %iv553_phi_vz_v4553, float %init_vz1_543, i32 1
  %iv555_phi_vz_v4555 = insertelement <4 x float> %iv554_phi_vz_v4554, float %init_vz2_545, i32 2
  %iv556_phi_vz_v4556 = insertelement <4 x float> %iv555_phi_vz_v4555, float %init_vz3_547, i32 3
  %iv557_phi_vx_v4557 = insertelement <4 x float> undef, float %init_vx0_521, i32 0
  %iv558_phi_vx_v4558 = insertelement <4 x float> %iv557_phi_vx_v4557, float %init_vx1_523, i32 1
  %iv559_phi_vx_v4559 = insertelement <4 x float> %iv558_phi_vx_v4558, float %init_vx2_525, i32 2
  %iv560_phi_vx_v4560 = insertelement <4 x float> %iv559_phi_vx_v4559, float %init_vx3_527, i32 3
  %iv561_phi_by_v4561 = insertelement <4 x float> undef, float %init_by0_499, i32 0
  %iv562_phi_by_v4562 = insertelement <4 x float> %iv561_phi_by_v4561, float %init_by1_501, i32 1
  %iv563_phi_by_v4563 = insertelement <4 x float> %iv562_phi_by_v4562, float %init_by2_503, i32 2
  %iv564_phi_by_v4564 = insertelement <4 x float> %iv563_phi_by_v4563, float %init_by3_505, i32 3
  %iv565_phi_bz_v4565 = insertelement <4 x float> undef, float %init_bz0_509, i32 0
  %iv566_phi_bz_v4566 = insertelement <4 x float> %iv565_phi_bz_v4565, float %init_bz1_511, i32 1
  %iv567_phi_bz_v4567 = insertelement <4 x float> %iv566_phi_bz_v4566, float %init_bz2_513, i32 2
  %iv568_phi_bz_v4568 = insertelement <4 x float> %iv567_phi_bz_v4567, float %init_bz3_515, i32 3
  %iv569_phi_vy_v4569 = insertelement <4 x float> undef, float %init_vy0_531, i32 0
  %iv570_phi_vy_v4570 = insertelement <4 x float> %iv569_phi_vy_v4569, float %init_vy1_533, i32 1
  %iv571_phi_vy_v4571 = insertelement <4 x float> %iv570_phi_vy_v4570, float %init_vy2_535, i32 2
  %iv572_phi_vy_v4572 = insertelement <4 x float> %iv571_phi_vy_v4571, float %init_vy3_537, i32 3
  %iv573_phi_bx_v4573 = insertelement <4 x float> undef, float %init_bx0_489, i32 0
  %iv574_phi_bx_v4574 = insertelement <4 x float> %iv573_phi_bx_v4573, float %init_bx1_491, i32 1
  %iv575_phi_bx_v4575 = insertelement <4 x float> %iv574_phi_bx_v4574, float %init_bx2_493, i32 2
  %iv576_phi_bx_v4576 = insertelement <4 x float> %iv575_phi_bx_v4575, float %init_bx3_495, i32 3
  br label %loop_hdr
loop_hdr:
  %pi_cnt_552 = phi i64 [ %init_count_551, %pre_phi ], [ %pn_cnt_552, %latch ]
  %phi_bx_v4 = phi <4 x float> [ %iv576_phi_bx_v4576, %pre_phi ], [ %be_bx_v4, %latch ]
  %phi_bz_v4 = phi <4 x float> [ %iv568_phi_bz_v4568, %pre_phi ], [ %be_bz_v4, %latch ]
  %phi_bz4 = phi float [ %init_bz4_517, %pre_phi ], [ %be_bz4, %latch ]
  %phi_bound = phi i64 [ %init_bound_487, %pre_phi ], [ %be_bound, %latch ]
  %phi_by4 = phi float [ %init_by4_507, %pre_phi ], [ %be_by4, %latch ]
  %phi_by_v4 = phi <4 x float> [ %iv564_phi_by_v4564, %pre_phi ], [ %be_by_v4, %latch ]
  %phi_vz_v4 = phi <4 x float> [ %iv556_phi_vz_v4556, %pre_phi ], [ %be_vz_v4, %latch ]
  %phi_vz4 = phi float [ %init_vz4_549, %pre_phi ], [ %be_vz4, %latch ]
  %phi_vx_v4 = phi <4 x float> [ %iv560_phi_vx_v4560, %pre_phi ], [ %be_vx_v4, %latch ]
  %phi_vy_v4 = phi <4 x float> [ %iv572_phi_vy_v4572, %pre_phi ], [ %be_vy_v4, %latch ]
  %phi_bx4 = phi float [ %init_bx4_497, %pre_phi ], [ %be_bx4, %latch ]
  %phi_cycle_count = phi i64 [ %init_cycle_count_519, %pre_phi ], [ %be_cycle_count, %latch ]
  %phi_vy4 = phi float [ %init_vy4_539, %pre_phi ], [ %be_vy4, %latch ]
  %phi_vx4 = phi float [ %init_vx4_529, %pre_phi ], [ %be_vx4, %latch ]
  %phi_count = phi i64 [ %init_count_551, %pre_phi ], [ %be_count, %latch ]
  %cmp_hdr_577 = icmp slt i64 %pi_cnt_552, %cnt_bound_220
  br i1 %cmp_hdr_577, label %body, label %commit
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
%t581 = fsub fast float %phi_bx_e0, %phi_bx_e1
  ; let dx01 = %t581
%t585 = fsub fast float %phi_by_e0, %phi_by_e1
  ; let dy01 = %t585
%t589 = fsub fast float %phi_bz_e0, %phi_bz_e1
  ; let dz01 = %t589
%t595 = fmul fast float %t581, %t581
%t599 = fmul fast float %t585, %t585
%t600 = fadd fast float %t595, %t599
%t604 = fmul fast float %t589, %t589
%t605 = fadd fast float %t600, %t604
  ; let dsq01 = %t605
  %t606 = call float @llvm.sqrt.f32(float %t605)
  ; let dist01 = %t606
  %il610 = load float, float* @dt, align 4
%t614 = fmul fast float %t605, %t606
%t615 = fdiv fast float %il610, %t614
  ; let mag01 = %t615
%t619 = fsub fast float %phi_bx_e0, %phi_bx_e2
  ; let dx02 = %t619
%t623 = fsub fast float %phi_by_e0, %phi_by_e2
  ; let dy02 = %t623
%t627 = fsub fast float %phi_bz_e0, %phi_bz_e2
  ; let dz02 = %t627
%t633 = fmul fast float %t619, %t619
%t637 = fmul fast float %t623, %t623
%t638 = fadd fast float %t633, %t637
%t642 = fmul fast float %t627, %t627
%t643 = fadd fast float %t638, %t642
  ; let dsq02 = %t643
  %t644 = call float @llvm.sqrt.f32(float %t643)
  ; let dist02 = %t644
  %il648 = load float, float* @dt, align 4
%t652 = fmul fast float %t643, %t644
%t653 = fdiv fast float %il648, %t652
  ; let mag02 = %t653
%t657 = fsub fast float %phi_bx_e0, %phi_bx_e3
  ; let dx03 = %t657
%t661 = fsub fast float %phi_by_e0, %phi_by_e3
  ; let dy03 = %t661
%t665 = fsub fast float %phi_bz_e0, %phi_bz_e3
  ; let dz03 = %t665
%t671 = fmul fast float %t657, %t657
%t675 = fmul fast float %t661, %t661
%t676 = fadd fast float %t671, %t675
%t680 = fmul fast float %t665, %t665
%t681 = fadd fast float %t676, %t680
  ; let dsq03 = %t681
  %t682 = call float @llvm.sqrt.f32(float %t681)
  ; let dist03 = %t682
  %il686 = load float, float* @dt, align 4
%t690 = fmul fast float %t681, %t682
%t691 = fdiv fast float %il686, %t690
  ; let mag03 = %t691
%t695 = fsub fast float %phi_bx_e0, %phi_bx4
  ; let dx04 = %t695
%t699 = fsub fast float %phi_by_e0, %phi_by4
  ; let dy04 = %t699
%t703 = fsub fast float %phi_bz_e0, %phi_bz4
  ; let dz04 = %t703
%t709 = fmul fast float %t695, %t695
%t713 = fmul fast float %t699, %t699
%t714 = fadd fast float %t709, %t713
%t718 = fmul fast float %t703, %t703
%t719 = fadd fast float %t714, %t718
  ; let dsq04 = %t719
  %t720 = call float @llvm.sqrt.f32(float %t719)
  ; let dist04 = %t720
  %il724 = load float, float* @dt, align 4
%t728 = fmul fast float %t719, %t720
%t729 = fdiv fast float %il724, %t728
  ; let mag04 = %t729
%t733 = fsub fast float %phi_bx_e1, %phi_bx_e2
  ; let dx12 = %t733
%t737 = fsub fast float %phi_by_e1, %phi_by_e2
  ; let dy12 = %t737
%t741 = fsub fast float %phi_bz_e1, %phi_bz_e2
  ; let dz12 = %t741
%t747 = fmul fast float %t733, %t733
%t751 = fmul fast float %t737, %t737
%t752 = fadd fast float %t747, %t751
%t756 = fmul fast float %t741, %t741
%t757 = fadd fast float %t752, %t756
  ; let dsq12 = %t757
  %t758 = call float @llvm.sqrt.f32(float %t757)
  ; let dist12 = %t758
  %il762 = load float, float* @dt, align 4
%t766 = fmul fast float %t757, %t758
%t767 = fdiv fast float %il762, %t766
  ; let mag12 = %t767
%t771 = fsub fast float %phi_bx_e1, %phi_bx_e3
  ; let dx13 = %t771
%t775 = fsub fast float %phi_by_e1, %phi_by_e3
  ; let dy13 = %t775
%t779 = fsub fast float %phi_bz_e1, %phi_bz_e3
  ; let dz13 = %t779
%t785 = fmul fast float %t771, %t771
%t789 = fmul fast float %t775, %t775
%t790 = fadd fast float %t785, %t789
%t794 = fmul fast float %t779, %t779
%t795 = fadd fast float %t790, %t794
  ; let dsq13 = %t795
  %t796 = call float @llvm.sqrt.f32(float %t795)
  ; let dist13 = %t796
  %il800 = load float, float* @dt, align 4
%t804 = fmul fast float %t795, %t796
%t805 = fdiv fast float %il800, %t804
  ; let mag13 = %t805
%t809 = fsub fast float %phi_bx_e1, %phi_bx4
  ; let dx14 = %t809
%t813 = fsub fast float %phi_by_e1, %phi_by4
  ; let dy14 = %t813
%t817 = fsub fast float %phi_bz_e1, %phi_bz4
  ; let dz14 = %t817
%t823 = fmul fast float %t809, %t809
%t827 = fmul fast float %t813, %t813
%t828 = fadd fast float %t823, %t827
%t832 = fmul fast float %t817, %t817
%t833 = fadd fast float %t828, %t832
  ; let dsq14 = %t833
  %t834 = call float @llvm.sqrt.f32(float %t833)
  ; let dist14 = %t834
  %il838 = load float, float* @dt, align 4
%t842 = fmul fast float %t833, %t834
%t843 = fdiv fast float %il838, %t842
  ; let mag14 = %t843
%t847 = fsub fast float %phi_bx_e2, %phi_bx_e3
  ; let dx23 = %t847
%t851 = fsub fast float %phi_by_e2, %phi_by_e3
  ; let dy23 = %t851
%t855 = fsub fast float %phi_bz_e2, %phi_bz_e3
  ; let dz23 = %t855
%t861 = fmul fast float %t847, %t847
%t865 = fmul fast float %t851, %t851
%t866 = fadd fast float %t861, %t865
%t870 = fmul fast float %t855, %t855
%t871 = fadd fast float %t866, %t870
  ; let dsq23 = %t871
  %t872 = call float @llvm.sqrt.f32(float %t871)
  ; let dist23 = %t872
  %il876 = load float, float* @dt, align 4
%t880 = fmul fast float %t871, %t872
%t881 = fdiv fast float %il876, %t880
  ; let mag23 = %t881
%t885 = fsub fast float %phi_bx_e2, %phi_bx4
  ; let dx24 = %t885
%t889 = fsub fast float %phi_by_e2, %phi_by4
  ; let dy24 = %t889
%t893 = fsub fast float %phi_bz_e2, %phi_bz4
  ; let dz24 = %t893
%t899 = fmul fast float %t885, %t885
%t903 = fmul fast float %t889, %t889
%t904 = fadd fast float %t899, %t903
%t908 = fmul fast float %t893, %t893
%t909 = fadd fast float %t904, %t908
  ; let dsq24 = %t909
  %t910 = call float @llvm.sqrt.f32(float %t909)
  ; let dist24 = %t910
  %il914 = load float, float* @dt, align 4
%t918 = fmul fast float %t909, %t910
%t919 = fdiv fast float %il914, %t918
  ; let mag24 = %t919
%t923 = fsub fast float %phi_bx_e3, %phi_bx4
  ; let dx34 = %t923
%t927 = fsub fast float %phi_by_e3, %phi_by4
  ; let dy34 = %t927
%t931 = fsub fast float %phi_bz_e3, %phi_bz4
  ; let dz34 = %t931
%t937 = fmul fast float %t923, %t923
%t941 = fmul fast float %t927, %t927
%t942 = fadd fast float %t937, %t941
%t946 = fmul fast float %t931, %t931
%t947 = fadd fast float %t942, %t946
  ; let dsq34 = %t947
  %t948 = call float @llvm.sqrt.f32(float %t947)
  ; let dist34 = %t948
  %il952 = load float, float* @dt, align 4
%t956 = fmul fast float %t947, %t948
%t957 = fdiv fast float %il952, %t956
  ; let mag34 = %t957
  %il967 = load float, float* @m1, align 4
%t968 = fmul fast float %t581, %il967
%t970 = fmul fast float %t968, %t615
%t971 = fsub fast float %phi_vx_e0, %t970
  %il976 = load float, float* @m2, align 4
%t977 = fmul fast float %t619, %il976
%t979 = fmul fast float %t977, %t653
%t980 = fsub fast float %t971, %t979
  %il985 = load float, float* @m3, align 4
%t986 = fmul fast float %t657, %il985
%t988 = fmul fast float %t986, %t691
%t989 = fsub fast float %t980, %t988
  %il994 = load float, float* @m4, align 4
%t995 = fmul fast float %t695, %il994
%t997 = fmul fast float %t995, %t729
%t998 = fsub fast float %t989, %t997
  ; let nvx0 = %t998
  %il1008 = load float, float* @m1, align 4
%t1009 = fmul fast float %t585, %il1008
%t1011 = fmul fast float %t1009, %t615
%t1012 = fsub fast float %phi_vy_e0, %t1011
  %il1017 = load float, float* @m2, align 4
%t1018 = fmul fast float %t623, %il1017
%t1020 = fmul fast float %t1018, %t653
%t1021 = fsub fast float %t1012, %t1020
  %il1026 = load float, float* @m3, align 4
%t1027 = fmul fast float %t661, %il1026
%t1029 = fmul fast float %t1027, %t691
%t1030 = fsub fast float %t1021, %t1029
  %il1035 = load float, float* @m4, align 4
%t1036 = fmul fast float %t699, %il1035
%t1038 = fmul fast float %t1036, %t729
%t1039 = fsub fast float %t1030, %t1038
  ; let nvy0 = %t1039
  %il1049 = load float, float* @m1, align 4
%t1050 = fmul fast float %t589, %il1049
%t1052 = fmul fast float %t1050, %t615
%t1053 = fsub fast float %phi_vz_e0, %t1052
  %il1058 = load float, float* @m2, align 4
%t1059 = fmul fast float %t627, %il1058
%t1061 = fmul fast float %t1059, %t653
%t1062 = fsub fast float %t1053, %t1061
  %il1067 = load float, float* @m3, align 4
%t1068 = fmul fast float %t665, %il1067
%t1070 = fmul fast float %t1068, %t691
%t1071 = fsub fast float %t1062, %t1070
  %il1076 = load float, float* @m4, align 4
%t1077 = fmul fast float %t703, %il1076
%t1079 = fmul fast float %t1077, %t729
%t1080 = fsub fast float %t1071, %t1079
  ; let nvz0 = %t1080
  %il1090 = load float, float* @m0, align 4
%t1091 = fmul fast float %t581, %il1090
%t1093 = fmul fast float %t1091, %t615
%t1094 = fadd fast float %phi_vx_e1, %t1093
  %il1099 = load float, float* @m2, align 4
%t1100 = fmul fast float %t733, %il1099
%t1102 = fmul fast float %t1100, %t767
%t1103 = fsub fast float %t1094, %t1102
  %il1108 = load float, float* @m3, align 4
%t1109 = fmul fast float %t771, %il1108
%t1111 = fmul fast float %t1109, %t805
%t1112 = fsub fast float %t1103, %t1111
  %il1117 = load float, float* @m4, align 4
%t1118 = fmul fast float %t809, %il1117
%t1120 = fmul fast float %t1118, %t843
%t1121 = fsub fast float %t1112, %t1120
  ; let nvx1 = %t1121
  %il1131 = load float, float* @m0, align 4
%t1132 = fmul fast float %t585, %il1131
%t1134 = fmul fast float %t1132, %t615
%t1135 = fadd fast float %phi_vy_e1, %t1134
  %il1140 = load float, float* @m2, align 4
%t1141 = fmul fast float %t737, %il1140
%t1143 = fmul fast float %t1141, %t767
%t1144 = fsub fast float %t1135, %t1143
  %il1149 = load float, float* @m3, align 4
%t1150 = fmul fast float %t775, %il1149
%t1152 = fmul fast float %t1150, %t805
%t1153 = fsub fast float %t1144, %t1152
  %il1158 = load float, float* @m4, align 4
%t1159 = fmul fast float %t813, %il1158
%t1161 = fmul fast float %t1159, %t843
%t1162 = fsub fast float %t1153, %t1161
  ; let nvy1 = %t1162
  %il1172 = load float, float* @m0, align 4
%t1173 = fmul fast float %t589, %il1172
%t1175 = fmul fast float %t1173, %t615
%t1176 = fadd fast float %phi_vz_e1, %t1175
  %il1181 = load float, float* @m2, align 4
%t1182 = fmul fast float %t741, %il1181
%t1184 = fmul fast float %t1182, %t767
%t1185 = fsub fast float %t1176, %t1184
  %il1190 = load float, float* @m3, align 4
%t1191 = fmul fast float %t779, %il1190
%t1193 = fmul fast float %t1191, %t805
%t1194 = fsub fast float %t1185, %t1193
  %il1199 = load float, float* @m4, align 4
%t1200 = fmul fast float %t817, %il1199
%t1202 = fmul fast float %t1200, %t843
%t1203 = fsub fast float %t1194, %t1202
  ; let nvz1 = %t1203
  %il1213 = load float, float* @m0, align 4
%t1214 = fmul fast float %t619, %il1213
%t1216 = fmul fast float %t1214, %t653
%t1217 = fadd fast float %phi_vx_e2, %t1216
  %il1222 = load float, float* @m1, align 4
%t1223 = fmul fast float %t733, %il1222
%t1225 = fmul fast float %t1223, %t767
%t1226 = fadd fast float %t1217, %t1225
  %il1231 = load float, float* @m3, align 4
%t1232 = fmul fast float %t847, %il1231
%t1234 = fmul fast float %t1232, %t881
%t1235 = fsub fast float %t1226, %t1234
  %il1240 = load float, float* @m4, align 4
%t1241 = fmul fast float %t885, %il1240
%t1243 = fmul fast float %t1241, %t919
%t1244 = fsub fast float %t1235, %t1243
  ; let nvx2 = %t1244
  %il1254 = load float, float* @m0, align 4
%t1255 = fmul fast float %t623, %il1254
%t1257 = fmul fast float %t1255, %t653
%t1258 = fadd fast float %phi_vy_e2, %t1257
  %il1263 = load float, float* @m1, align 4
%t1264 = fmul fast float %t737, %il1263
%t1266 = fmul fast float %t1264, %t767
%t1267 = fadd fast float %t1258, %t1266
  %il1272 = load float, float* @m3, align 4
%t1273 = fmul fast float %t851, %il1272
%t1275 = fmul fast float %t1273, %t881
%t1276 = fsub fast float %t1267, %t1275
  %il1281 = load float, float* @m4, align 4
%t1282 = fmul fast float %t889, %il1281
%t1284 = fmul fast float %t1282, %t919
%t1285 = fsub fast float %t1276, %t1284
  ; let nvy2 = %t1285
  %il1295 = load float, float* @m0, align 4
%t1296 = fmul fast float %t627, %il1295
%t1298 = fmul fast float %t1296, %t653
%t1299 = fadd fast float %phi_vz_e2, %t1298
  %il1304 = load float, float* @m1, align 4
%t1305 = fmul fast float %t741, %il1304
%t1307 = fmul fast float %t1305, %t767
%t1308 = fadd fast float %t1299, %t1307
  %il1313 = load float, float* @m3, align 4
%t1314 = fmul fast float %t855, %il1313
%t1316 = fmul fast float %t1314, %t881
%t1317 = fsub fast float %t1308, %t1316
  %il1322 = load float, float* @m4, align 4
%t1323 = fmul fast float %t893, %il1322
%t1325 = fmul fast float %t1323, %t919
%t1326 = fsub fast float %t1317, %t1325
  ; let nvz2 = %t1326
  %il1336 = load float, float* @m0, align 4
%t1337 = fmul fast float %t657, %il1336
%t1339 = fmul fast float %t1337, %t691
%t1340 = fadd fast float %phi_vx_e3, %t1339
  %il1345 = load float, float* @m1, align 4
%t1346 = fmul fast float %t771, %il1345
%t1348 = fmul fast float %t1346, %t805
%t1349 = fadd fast float %t1340, %t1348
  %il1354 = load float, float* @m2, align 4
%t1355 = fmul fast float %t847, %il1354
%t1357 = fmul fast float %t1355, %t881
%t1358 = fadd fast float %t1349, %t1357
  %il1363 = load float, float* @m4, align 4
%t1364 = fmul fast float %t923, %il1363
%t1366 = fmul fast float %t1364, %t957
%t1367 = fsub fast float %t1358, %t1366
  ; let nvx3 = %t1367
  %il1377 = load float, float* @m0, align 4
%t1378 = fmul fast float %t661, %il1377
%t1380 = fmul fast float %t1378, %t691
%t1381 = fadd fast float %phi_vy_e3, %t1380
  %il1386 = load float, float* @m1, align 4
%t1387 = fmul fast float %t775, %il1386
%t1389 = fmul fast float %t1387, %t805
%t1390 = fadd fast float %t1381, %t1389
  %il1395 = load float, float* @m2, align 4
%t1396 = fmul fast float %t851, %il1395
%t1398 = fmul fast float %t1396, %t881
%t1399 = fadd fast float %t1390, %t1398
  %il1404 = load float, float* @m4, align 4
%t1405 = fmul fast float %t927, %il1404
%t1407 = fmul fast float %t1405, %t957
%t1408 = fsub fast float %t1399, %t1407
  ; let nvy3 = %t1408
  %il1418 = load float, float* @m0, align 4
%t1419 = fmul fast float %t665, %il1418
%t1421 = fmul fast float %t1419, %t691
%t1422 = fadd fast float %phi_vz_e3, %t1421
  %il1427 = load float, float* @m1, align 4
%t1428 = fmul fast float %t779, %il1427
%t1430 = fmul fast float %t1428, %t805
%t1431 = fadd fast float %t1422, %t1430
  %il1436 = load float, float* @m2, align 4
%t1437 = fmul fast float %t855, %il1436
%t1439 = fmul fast float %t1437, %t881
%t1440 = fadd fast float %t1431, %t1439
  %il1445 = load float, float* @m4, align 4
%t1446 = fmul fast float %t931, %il1445
%t1448 = fmul fast float %t1446, %t957
%t1449 = fsub fast float %t1440, %t1448
  ; let nvz3 = %t1449
  %il1459 = load float, float* @m0, align 4
%t1460 = fmul fast float %t695, %il1459
%t1462 = fmul fast float %t1460, %t729
%t1463 = fadd fast float %phi_vx4, %t1462
  %il1468 = load float, float* @m1, align 4
%t1469 = fmul fast float %t809, %il1468
%t1471 = fmul fast float %t1469, %t843
%t1472 = fadd fast float %t1463, %t1471
  %il1477 = load float, float* @m2, align 4
%t1478 = fmul fast float %t885, %il1477
%t1480 = fmul fast float %t1478, %t919
%t1481 = fadd fast float %t1472, %t1480
  %il1486 = load float, float* @m3, align 4
%t1487 = fmul fast float %t923, %il1486
%t1489 = fmul fast float %t1487, %t957
%t1490 = fadd fast float %t1481, %t1489
  ; let nvx4 = %t1490
  %il1500 = load float, float* @m0, align 4
%t1501 = fmul fast float %t699, %il1500
%t1503 = fmul fast float %t1501, %t729
%t1504 = fadd fast float %phi_vy4, %t1503
  %il1509 = load float, float* @m1, align 4
%t1510 = fmul fast float %t813, %il1509
%t1512 = fmul fast float %t1510, %t843
%t1513 = fadd fast float %t1504, %t1512
  %il1518 = load float, float* @m2, align 4
%t1519 = fmul fast float %t889, %il1518
%t1521 = fmul fast float %t1519, %t919
%t1522 = fadd fast float %t1513, %t1521
  %il1527 = load float, float* @m3, align 4
%t1528 = fmul fast float %t927, %il1527
%t1530 = fmul fast float %t1528, %t957
%t1531 = fadd fast float %t1522, %t1530
  ; let nvy4 = %t1531
  %il1541 = load float, float* @m0, align 4
%t1542 = fmul fast float %t703, %il1541
%t1544 = fmul fast float %t1542, %t729
%t1545 = fadd fast float %phi_vz4, %t1544
  %il1550 = load float, float* @m1, align 4
%t1551 = fmul fast float %t817, %il1550
%t1553 = fmul fast float %t1551, %t843
%t1554 = fadd fast float %t1545, %t1553
  %il1559 = load float, float* @m2, align 4
%t1560 = fmul fast float %t893, %il1559
%t1562 = fmul fast float %t1560, %t919
%t1563 = fadd fast float %t1554, %t1562
  %il1568 = load float, float* @m3, align 4
%t1569 = fmul fast float %t931, %il1568
%t1571 = fmul fast float %t1569, %t957
%t1572 = fadd fast float %t1563, %t1571
  ; let nvz4 = %t1572
   %iv1574_phi_vx_v4 = insertelement <4 x float> %phi_vx_v4, float %t998, i32 0
   %iv1576_phi_vy_v4 = insertelement <4 x float> %phi_vy_v4, float %t1039, i32 0
   %iv1578_phi_vz_v4 = insertelement <4 x float> %phi_vz_v4, float %t1080, i32 0
   %iv1580_phi_vx_v4 = insertelement <4 x float> %iv1574_phi_vx_v4, float %t1121, i32 1
   %iv1582_phi_vy_v4 = insertelement <4 x float> %iv1576_phi_vy_v4, float %t1162, i32 1
   %iv1584_phi_vz_v4 = insertelement <4 x float> %iv1578_phi_vz_v4, float %t1203, i32 1
   %iv1586_phi_vx_v4 = insertelement <4 x float> %iv1580_phi_vx_v4, float %t1244, i32 2
   %iv1588_phi_vy_v4 = insertelement <4 x float> %iv1582_phi_vy_v4, float %t1285, i32 2
   %iv1590_phi_vz_v4 = insertelement <4 x float> %iv1584_phi_vz_v4, float %t1326, i32 2
   %iv1592_phi_vx_v4 = insertelement <4 x float> %iv1586_phi_vx_v4, float %t1367, i32 3
   %iv1594_phi_vy_v4 = insertelement <4 x float> %iv1588_phi_vy_v4, float %t1408, i32 3
   %iv1596_phi_vz_v4 = insertelement <4 x float> %iv1590_phi_vz_v4, float %t1449, i32 3
  %ap_1598 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 14
  %ap_1600 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 0
  %ap_1602 = getelementptr inbounds %StateChunk2, ptr %state_2, i32 0, i32 1
  %il1607 = load float, float* @dt, align 4
%t1609 = fmul fast float %il1607, %t998
%t1610 = fadd fast float %phi_bx_e0, %t1609
   %iv1611_phi_bx_v4 = insertelement <4 x float> %phi_bx_v4, float %t1610, i32 0
  %il1616 = load float, float* @dt, align 4
%t1618 = fmul fast float %il1616, %t1039
%t1619 = fadd fast float %phi_by_e0, %t1618
   %iv1620_phi_by_v4 = insertelement <4 x float> %phi_by_v4, float %t1619, i32 0
  %il1625 = load float, float* @dt, align 4
%t1627 = fmul fast float %il1625, %t1080
%t1628 = fadd fast float %phi_bz_e0, %t1627
   %iv1629_phi_bz_v4 = insertelement <4 x float> %phi_bz_v4, float %t1628, i32 0
  %il1634 = load float, float* @dt, align 4
%t1636 = fmul fast float %il1634, %t1121
%t1637 = fadd fast float %phi_bx_e1, %t1636
   %iv1638_phi_bx_v4 = insertelement <4 x float> %iv1611_phi_bx_v4, float %t1637, i32 1
  %il1643 = load float, float* @dt, align 4
%t1645 = fmul fast float %il1643, %t1162
%t1646 = fadd fast float %phi_by_e1, %t1645
   %iv1647_phi_by_v4 = insertelement <4 x float> %iv1620_phi_by_v4, float %t1646, i32 1
  %il1652 = load float, float* @dt, align 4
%t1654 = fmul fast float %il1652, %t1203
%t1655 = fadd fast float %phi_bz_e1, %t1654
   %iv1656_phi_bz_v4 = insertelement <4 x float> %iv1629_phi_bz_v4, float %t1655, i32 1
  %il1661 = load float, float* @dt, align 4
%t1663 = fmul fast float %il1661, %t1244
%t1664 = fadd fast float %phi_bx_e2, %t1663
   %iv1665_phi_bx_v4 = insertelement <4 x float> %iv1638_phi_bx_v4, float %t1664, i32 2
  %il1670 = load float, float* @dt, align 4
%t1672 = fmul fast float %il1670, %t1285
%t1673 = fadd fast float %phi_by_e2, %t1672
   %iv1674_phi_by_v4 = insertelement <4 x float> %iv1647_phi_by_v4, float %t1673, i32 2
  %il1679 = load float, float* @dt, align 4
%t1681 = fmul fast float %il1679, %t1326
%t1682 = fadd fast float %phi_bz_e2, %t1681
   %iv1683_phi_bz_v4 = insertelement <4 x float> %iv1656_phi_bz_v4, float %t1682, i32 2
  %il1688 = load float, float* @dt, align 4
%t1690 = fmul fast float %il1688, %t1367
%t1691 = fadd fast float %phi_bx_e3, %t1690
   %iv1692_phi_bx_v4 = insertelement <4 x float> %iv1665_phi_bx_v4, float %t1691, i32 3
  %il1697 = load float, float* @dt, align 4
%t1699 = fmul fast float %il1697, %t1408
%t1700 = fadd fast float %phi_by_e3, %t1699
   %iv1701_phi_by_v4 = insertelement <4 x float> %iv1674_phi_by_v4, float %t1700, i32 3
  %il1706 = load float, float* @dt, align 4
%t1708 = fmul fast float %il1706, %t1449
%t1709 = fadd fast float %phi_bz_e3, %t1708
   %iv1710_phi_bz_v4 = insertelement <4 x float> %iv1683_phi_bz_v4, float %t1709, i32 3
  %il1715 = load float, float* @dt, align 4
%t1717 = fmul fast float %il1715, %t1490
%t1718 = fadd fast float %phi_bx4, %t1717
  %ap_1719 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 11
  %il1724 = load float, float* @dt, align 4
%t1726 = fmul fast float %il1724, %t1531
%t1727 = fadd fast float %phi_by4, %t1726
  %ap_1728 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 12
  %il1733 = load float, float* @dt, align 4
%t1735 = fmul fast float %il1733, %t1572
%t1736 = fadd fast float %phi_bz4, %t1735
  %ap_1737 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 13
  br label %latch
latch:
  %pn_cnt_552 = add i64 %pi_cnt_552, 1
  %be_bound = add i64 0, %phi_bound
  %be_bx_v4 = bitcast <4 x float> %iv1692_phi_bx_v4 to <4 x float>
  %be_bx4 = fadd float %t1718, 0.0
  %be_by_v4 = bitcast <4 x float> %iv1701_phi_by_v4 to <4 x float>
  %be_by4 = fadd float %t1727, 0.0
  %be_bz_v4 = bitcast <4 x float> %iv1710_phi_bz_v4 to <4 x float>
  %be_bz4 = fadd float %t1736, 0.0
  %be_cycle_count = add i64 0, %phi_cycle_count
  %be_vx_v4 = bitcast <4 x float> %iv1592_phi_vx_v4 to <4 x float>
  %be_vx4 = fadd float %t1490, 0.0
  %be_vy_v4 = bitcast <4 x float> %iv1594_phi_vy_v4 to <4 x float>
  %be_vy4 = fadd float %t1531, 0.0
  %be_vz_v4 = bitcast <4 x float> %iv1596_phi_vz_v4 to <4 x float>
  %be_vz4 = fadd float %t1572, 0.0
  %be_count = add i64 0, %pn_cnt_552
  br label %loop_hdr, !llvm.loop !100
commit:
  store <4 x float> %phi_bx_v4, ptr %lv_474, align 16
  store float %phi_bx4, ptr %lv_479, align 4
  store <4 x float> %phi_by_v4, ptr %lv_475, align 16
  store float %phi_by4, ptr %lv_485, align 4
  store <4 x float> %phi_bz_v4, ptr %lv_478, align 16
  store float %phi_bz4, ptr %lv_480, align 4
  store <4 x float> %phi_vx_v4, ptr %lv_481, align 16
  store float %phi_vx4, ptr %lv_483, align 4
  store <4 x float> %phi_vy_v4, ptr %lv_482, align 16
  store float %phi_vy4, ptr %lv_476, align 4
  store <4 x float> %phi_vz_v4, ptr %lv_477, align 16
  store float %phi_vz4, ptr %lv_484, align 4
  br label %done
done:
  %arr4 = load ptr, ptr %arbase3, align 8
  store i8* %arr4, ptr %arptr3, align 8
  %lv_1738 = load <4 x float>, ptr %lv_474, align 16
  %lve_1739 = extractelement <4 x float> %lv_1738, i32 0
  %lve_1740 = extractelement <4 x float> %lv_1738, i32 1
  %lve_1741 = extractelement <4 x float> %lv_1738, i32 2
  %lve_1742 = extractelement <4 x float> %lv_1738, i32 3
  %lv_1743 = load float, ptr %lv_479, align 4
  %lv_1744 = load <4 x float>, ptr %lv_475, align 16
  %lve_1745 = extractelement <4 x float> %lv_1744, i32 0
  %lve_1746 = extractelement <4 x float> %lv_1744, i32 1
  %lve_1747 = extractelement <4 x float> %lv_1744, i32 2
  %lve_1748 = extractelement <4 x float> %lv_1744, i32 3
  %lv_1749 = load float, ptr %lv_485, align 4
  %lv_1750 = load <4 x float>, ptr %lv_478, align 16
  %lve_1751 = extractelement <4 x float> %lv_1750, i32 0
  %lve_1752 = extractelement <4 x float> %lv_1750, i32 1
  %lve_1753 = extractelement <4 x float> %lv_1750, i32 2
  %lve_1754 = extractelement <4 x float> %lv_1750, i32 3
  %lv_1755 = load float, ptr %lv_480, align 4
  %lv_1756 = load <4 x float>, ptr %lv_481, align 16
  %lve_1757 = extractelement <4 x float> %lv_1756, i32 0
  %lve_1758 = extractelement <4 x float> %lv_1756, i32 1
  %lve_1759 = extractelement <4 x float> %lv_1756, i32 2
  %lve_1760 = extractelement <4 x float> %lv_1756, i32 3
  %lv_1761 = load float, ptr %lv_483, align 4
  %lv_1762 = load <4 x float>, ptr %lv_482, align 16
  %lve_1763 = extractelement <4 x float> %lv_1762, i32 0
  %lve_1764 = extractelement <4 x float> %lv_1762, i32 1
  %lve_1765 = extractelement <4 x float> %lv_1762, i32 2
  %lve_1766 = extractelement <4 x float> %lv_1762, i32 3
  %lv_1767 = load float, ptr %lv_476, align 4
  %lv_1768 = load <4 x float>, ptr %lv_477, align 16
  %lve_1769 = extractelement <4 x float> %lv_1768, i32 0
  %lve_1770 = extractelement <4 x float> %lv_1768, i32 1
  %lve_1771 = extractelement <4 x float> %lv_1768, i32 2
  %lve_1772 = extractelement <4 x float> %lv_1768, i32 3
  %lv_1773 = load float, ptr %lv_484, align 4
%t1781 = fsub fast float %lve_1739, %lve_1740
%t1785 = fsub fast float %lve_1739, %lve_1740
%t1786 = fmul fast float %t1781, %t1785
%t1791 = fsub fast float %lve_1745, %lve_1746
%t1795 = fsub fast float %lve_1745, %lve_1746
%t1796 = fmul fast float %t1791, %t1795
%t1797 = fadd fast float %t1786, %t1796
%t1802 = fsub fast float %lve_1751, %lve_1752
%t1806 = fsub fast float %lve_1751, %lve_1752
%t1807 = fmul fast float %t1802, %t1806
%t1808 = fadd fast float %t1797, %t1807
  %t1774 = call float @llvm.sqrt.f32(float %t1808)
  ; let edist01 = %t1774
%t1816 = fsub fast float %lve_1739, %lve_1741
%t1820 = fsub fast float %lve_1739, %lve_1741
%t1821 = fmul fast float %t1816, %t1820
%t1826 = fsub fast float %lve_1745, %lve_1747
%t1830 = fsub fast float %lve_1745, %lve_1747
%t1831 = fmul fast float %t1826, %t1830
%t1832 = fadd fast float %t1821, %t1831
%t1837 = fsub fast float %lve_1751, %lve_1753
%t1841 = fsub fast float %lve_1751, %lve_1753
%t1842 = fmul fast float %t1837, %t1841
%t1843 = fadd fast float %t1832, %t1842
  %t1809 = call float @llvm.sqrt.f32(float %t1843)
  ; let edist02 = %t1809
%t1851 = fsub fast float %lve_1739, %lve_1742
%t1855 = fsub fast float %lve_1739, %lve_1742
%t1856 = fmul fast float %t1851, %t1855
%t1861 = fsub fast float %lve_1745, %lve_1748
%t1865 = fsub fast float %lve_1745, %lve_1748
%t1866 = fmul fast float %t1861, %t1865
%t1867 = fadd fast float %t1856, %t1866
%t1872 = fsub fast float %lve_1751, %lve_1754
%t1876 = fsub fast float %lve_1751, %lve_1754
%t1877 = fmul fast float %t1872, %t1876
%t1878 = fadd fast float %t1867, %t1877
  %t1844 = call float @llvm.sqrt.f32(float %t1878)
  ; let edist03 = %t1844
%t1886 = fsub fast float %lve_1739, %lv_1743
%t1890 = fsub fast float %lve_1739, %lv_1743
%t1891 = fmul fast float %t1886, %t1890
%t1896 = fsub fast float %lve_1745, %lv_1749
%t1900 = fsub fast float %lve_1745, %lv_1749
%t1901 = fmul fast float %t1896, %t1900
%t1902 = fadd fast float %t1891, %t1901
%t1907 = fsub fast float %lve_1751, %lv_1755
%t1911 = fsub fast float %lve_1751, %lv_1755
%t1912 = fmul fast float %t1907, %t1911
%t1913 = fadd fast float %t1902, %t1912
  %t1879 = call float @llvm.sqrt.f32(float %t1913)
  ; let edist04 = %t1879
%t1921 = fsub fast float %lve_1740, %lve_1741
%t1925 = fsub fast float %lve_1740, %lve_1741
%t1926 = fmul fast float %t1921, %t1925
%t1931 = fsub fast float %lve_1746, %lve_1747
%t1935 = fsub fast float %lve_1746, %lve_1747
%t1936 = fmul fast float %t1931, %t1935
%t1937 = fadd fast float %t1926, %t1936
%t1942 = fsub fast float %lve_1752, %lve_1753
%t1946 = fsub fast float %lve_1752, %lve_1753
%t1947 = fmul fast float %t1942, %t1946
%t1948 = fadd fast float %t1937, %t1947
  %t1914 = call float @llvm.sqrt.f32(float %t1948)
  ; let edist12 = %t1914
%t1956 = fsub fast float %lve_1740, %lve_1742
%t1960 = fsub fast float %lve_1740, %lve_1742
%t1961 = fmul fast float %t1956, %t1960
%t1966 = fsub fast float %lve_1746, %lve_1748
%t1970 = fsub fast float %lve_1746, %lve_1748
%t1971 = fmul fast float %t1966, %t1970
%t1972 = fadd fast float %t1961, %t1971
%t1977 = fsub fast float %lve_1752, %lve_1754
%t1981 = fsub fast float %lve_1752, %lve_1754
%t1982 = fmul fast float %t1977, %t1981
%t1983 = fadd fast float %t1972, %t1982
  %t1949 = call float @llvm.sqrt.f32(float %t1983)
  ; let edist13 = %t1949
%t1991 = fsub fast float %lve_1740, %lv_1743
%t1995 = fsub fast float %lve_1740, %lv_1743
%t1996 = fmul fast float %t1991, %t1995
%t2001 = fsub fast float %lve_1746, %lv_1749
%t2005 = fsub fast float %lve_1746, %lv_1749
%t2006 = fmul fast float %t2001, %t2005
%t2007 = fadd fast float %t1996, %t2006
%t2012 = fsub fast float %lve_1752, %lv_1755
%t2016 = fsub fast float %lve_1752, %lv_1755
%t2017 = fmul fast float %t2012, %t2016
%t2018 = fadd fast float %t2007, %t2017
  %t1984 = call float @llvm.sqrt.f32(float %t2018)
  ; let edist14 = %t1984
%t2026 = fsub fast float %lve_1741, %lve_1742
%t2030 = fsub fast float %lve_1741, %lve_1742
%t2031 = fmul fast float %t2026, %t2030
%t2036 = fsub fast float %lve_1747, %lve_1748
%t2040 = fsub fast float %lve_1747, %lve_1748
%t2041 = fmul fast float %t2036, %t2040
%t2042 = fadd fast float %t2031, %t2041
%t2047 = fsub fast float %lve_1753, %lve_1754
%t2051 = fsub fast float %lve_1753, %lve_1754
%t2052 = fmul fast float %t2047, %t2051
%t2053 = fadd fast float %t2042, %t2052
  %t2019 = call float @llvm.sqrt.f32(float %t2053)
  ; let edist23 = %t2019
%t2061 = fsub fast float %lve_1741, %lv_1743
%t2065 = fsub fast float %lve_1741, %lv_1743
%t2066 = fmul fast float %t2061, %t2065
%t2071 = fsub fast float %lve_1747, %lv_1749
%t2075 = fsub fast float %lve_1747, %lv_1749
%t2076 = fmul fast float %t2071, %t2075
%t2077 = fadd fast float %t2066, %t2076
%t2082 = fsub fast float %lve_1753, %lv_1755
%t2086 = fsub fast float %lve_1753, %lv_1755
%t2087 = fmul fast float %t2082, %t2086
%t2088 = fadd fast float %t2077, %t2087
  %t2054 = call float @llvm.sqrt.f32(float %t2088)
  ; let edist24 = %t2054
%t2096 = fsub fast float %lve_1742, %lv_1743
%t2100 = fsub fast float %lve_1742, %lv_1743
%t2101 = fmul fast float %t2096, %t2100
%t2106 = fsub fast float %lve_1748, %lv_1749
%t2110 = fsub fast float %lve_1748, %lv_1749
%t2111 = fmul fast float %t2106, %t2110
%t2112 = fadd fast float %t2101, %t2111
%t2117 = fsub fast float %lve_1754, %lv_1755
%t2121 = fsub fast float %lve_1754, %lv_1755
%t2122 = fmul fast float %t2117, %t2121
%t2123 = fadd fast float %t2112, %t2122
  %t2089 = call float @llvm.sqrt.f32(float %t2123)
  ; let edist34 = %t2089
  %il2127 = load float, float* @m0, align 4
  %il2129 = load float, float* @m1, align 4
%t2130 = fmul fast float %il2127, %il2129
%t2132 = fdiv fast float %t2130, %t1774
  ; let e01 = %t2132
  %il2136 = load float, float* @m0, align 4
  %il2138 = load float, float* @m2, align 4
%t2139 = fmul fast float %il2136, %il2138
%t2141 = fdiv fast float %t2139, %t1809
  ; let e02 = %t2141
  %il2145 = load float, float* @m0, align 4
  %il2147 = load float, float* @m3, align 4
%t2148 = fmul fast float %il2145, %il2147
%t2150 = fdiv fast float %t2148, %t1844
  ; let e03 = %t2150
  %il2154 = load float, float* @m0, align 4
  %il2156 = load float, float* @m4, align 4
%t2157 = fmul fast float %il2154, %il2156
%t2159 = fdiv fast float %t2157, %t1879
  ; let e04 = %t2159
  %il2163 = load float, float* @m1, align 4
  %il2165 = load float, float* @m2, align 4
%t2166 = fmul fast float %il2163, %il2165
%t2168 = fdiv fast float %t2166, %t1914
  ; let e12 = %t2168
  %il2172 = load float, float* @m1, align 4
  %il2174 = load float, float* @m3, align 4
%t2175 = fmul fast float %il2172, %il2174
%t2177 = fdiv fast float %t2175, %t1949
  ; let e13 = %t2177
  %il2181 = load float, float* @m1, align 4
  %il2183 = load float, float* @m4, align 4
%t2184 = fmul fast float %il2181, %il2183
%t2186 = fdiv fast float %t2184, %t1984
  ; let e14 = %t2186
  %il2190 = load float, float* @m2, align 4
  %il2192 = load float, float* @m3, align 4
%t2193 = fmul fast float %il2190, %il2192
%t2195 = fdiv fast float %t2193, %t2019
  ; let e23 = %t2195
  %il2199 = load float, float* @m2, align 4
  %il2201 = load float, float* @m4, align 4
%t2202 = fmul fast float %il2199, %il2201
%t2204 = fdiv fast float %t2202, %t2054
  ; let e24 = %t2204
  %il2208 = load float, float* @m3, align 4
  %il2210 = load float, float* @m4, align 4
%t2211 = fmul fast float %il2208, %il2210
%t2213 = fdiv fast float %t2211, %t2089
  ; let e34 = %t2213
%t2227 = fadd fast float %t2132, %t2141
%t2229 = fadd fast float %t2227, %t2150
%t2231 = fadd fast float %t2229, %t2159
%t2233 = fadd fast float %t2231, %t2168
%t2235 = fadd fast float %t2233, %t2177
%t2237 = fadd fast float %t2235, %t2186
%t2239 = fadd fast float %t2237, %t2195
%t2241 = fadd fast float %t2239, %t2204
%t2243 = fadd fast float %t2241, %t2213
  %t2244 = fneg float %t2243
  ; let ep = %t2244
%ff2249 = bitcast i32 1056964608 to float
  %il2251 = load float, float* @m0, align 4
%t2252 = fmul fast float %ff2249, %il2251
%t2258 = fmul fast float %lve_1757, %lve_1757
%t2262 = fmul fast float %lve_1763, %lve_1763
%t2263 = fadd fast float %t2258, %t2262
%t2267 = fmul fast float %lve_1769, %lve_1769
%t2268 = fadd fast float %t2263, %t2267
%t2269 = fmul fast float %t2252, %t2268
  ; let ek0 = %t2269
%ff2274 = bitcast i32 1056964608 to float
  %il2276 = load float, float* @m1, align 4
%t2277 = fmul fast float %ff2274, %il2276
%t2283 = fmul fast float %lve_1758, %lve_1758
%t2287 = fmul fast float %lve_1764, %lve_1764
%t2288 = fadd fast float %t2283, %t2287
%t2292 = fmul fast float %lve_1770, %lve_1770
%t2293 = fadd fast float %t2288, %t2292
%t2294 = fmul fast float %t2277, %t2293
  ; let ek1 = %t2294
%ff2299 = bitcast i32 1056964608 to float
  %il2301 = load float, float* @m2, align 4
%t2302 = fmul fast float %ff2299, %il2301
%t2308 = fmul fast float %lve_1759, %lve_1759
%t2312 = fmul fast float %lve_1765, %lve_1765
%t2313 = fadd fast float %t2308, %t2312
%t2317 = fmul fast float %lve_1771, %lve_1771
%t2318 = fadd fast float %t2313, %t2317
%t2319 = fmul fast float %t2302, %t2318
  ; let ek2 = %t2319
%ff2324 = bitcast i32 1056964608 to float
  %il2326 = load float, float* @m3, align 4
%t2327 = fmul fast float %ff2324, %il2326
%t2333 = fmul fast float %lve_1760, %lve_1760
%t2337 = fmul fast float %lve_1766, %lve_1766
%t2338 = fadd fast float %t2333, %t2337
%t2342 = fmul fast float %lve_1772, %lve_1772
%t2343 = fadd fast float %t2338, %t2342
%t2344 = fmul fast float %t2327, %t2343
  ; let ek3 = %t2344
%ff2349 = bitcast i32 1056964608 to float
  %il2351 = load float, float* @m4, align 4
%t2352 = fmul fast float %ff2349, %il2351
%t2358 = fmul fast float %lv_1761, %lv_1761
%t2362 = fmul fast float %lv_1767, %lv_1767
%t2363 = fadd fast float %t2358, %t2362
%t2367 = fmul fast float %lv_1773, %lv_1773
%t2368 = fadd fast float %t2363, %t2367
%t2369 = fmul fast float %t2352, %t2368
  ; let ek4 = %t2369
%t2377 = fadd fast float %t2244, %t2269
%t2379 = fadd fast float %t2377, %t2294
%t2381 = fadd fast float %t2379, %t2319
%t2383 = fadd fast float %t2381, %t2344
%t2385 = fadd fast float %t2383, %t2369
  ; let energy = %t2385
  %pfd2388 = fpext float %t2385 to double
  %pso2389 = load volatile ptr, ptr @stdout
  %pff2390 = getelementptr [6 x i8], [6 x i8]* @FMT_FLOAT, i64 0, i64 0
  %ppf2391 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso2389, ptr %pff2390, double %pfd2388)
  %t2386 = zext i32 %ppf2391 to i64
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

!0 = !{!"Brief"}
!1 = !{!"Int", !0}
!2 = !{!"Char", !0}
!3 = !{!"Float", !0}
!4 = !{!"Bool", !0}
!5 = !{!"String", !0}
!99 = distinct !{} ; StateAliasScope
