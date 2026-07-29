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
@IC = constant i64 29573
@IA = constant i64 3877
@IM = constant i64 139968

%StateChunk0 = type { i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64 }
%StateChunk1 = type { i64, i64, i64 }
%State = type { i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64 }
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

define void @fan(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp2 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t1 = load i64, i64* %fdp2, align 8
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t3 = load i64, i64* %fdp4, align 8
%c5 = icmp slt i64 %t1, %t3
  %pi6 = and i1 %c5, true
  br i1 %pi6, label %ps8, label %pp7
  pp7:
    unreachable
  ps8:
  call void @llvm.assume(i1 %pi6)
  %fdp13 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  %t12 = load i64, i64* %fdp13, align 8
  %il15 = load i64, i64* @IA, align 8
  %t14 = add i64 0, %il15
%t16 = mul nsw i64 %t12, %t14
  %il18 = load i64, i64* @IC, align 8
  %t17 = add i64 0, %il18
%t19 = add nsw i64 %t16, %t17
  %il21 = load i64, i64* @IM, align 8
  %t20 = add i64 0, %il21
%t22 = srem i64 %t19, %t20
  ; let ns = %t22
  %fdp24 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %t23 = load i64, i64* %fdp24, align 8
  ; let saved = %t23
  %fdp27 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t26 = load i64, i64* %fdp27, align 8
%t29 = add i64 0, 1
%t30 = add nsw i64 %t26, %t29
  %ap_31 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t30, ptr %ap_31, align 8, !tbaa !1
  %t32 = add i64 0, %t22
  %ap_33 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 %t32, ptr %ap_33, align 8, !tbaa !1
  %fdp35 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %t34 = load i64, i64* %fdp35, align 8
  %ap_36 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store i64 %t34, ptr %ap_36, align 8, !tbaa !1
  %fdp39 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %t38 = load i64, i64* %fdp39, align 8
  %t41 = add i64 0, %t23
%t43 = add i64 0, 13
%t44 = srem i64 %t41, %t43
%t45 = add nsw i64 %t38, %t44
  ; let nchecksum = %t45
  %fdp47 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  %t46 = load i64, i64* %fdp47, align 8
  %ap_48 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 %t46, ptr %ap_48, align 8, !tbaa !1
  %t49 = add i64 0, %t45
  %ap_50 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 %t49, ptr %ap_50, align 8, !tbaa !1
  %fdp53 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %t52 = load i64, i64* %fdp53, align 8
  %t55 = add i64 0, %t45
%t57 = add i64 0, 17
%t58 = srem i64 %t55, %t57
%t59 = add nsw i64 %t52, %t58
  ; let nmax = %t59
  %fdp61 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  %t60 = load i64, i64* %fdp61, align 8
  %ap_62 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
  store i64 %t60, ptr %ap_62, align 8, !tbaa !1
  %fdp65 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t64 = load i64, i64* %fdp65, align 8
  %fdp67 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t66 = load i64, i64* %fdp67, align 8
%c68 = icmp eq i64 %t64, %t66
  br i1 %c68, label %g69_t, label %g69_e
  g69_t:
    %fdp72 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
    %t71 = load i64, i64* %fdp72, align 8
    %pso73 = load volatile ptr, ptr @stdout
    %pfi74 = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0
    %ppi75 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso73, ptr %pfi74, i64 %t71)
    %t70 = zext i32 %ppi75 to i64
    ret void
  g69_e:
  %t76 = add i64 0, %t59
  %ap_77 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 %t76, ptr %ap_77, align 8, !tbaa !1
  %fdp79 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  %t78 = load i64, i64* %fdp79, align 8
  %ap_80 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
  store i64 %t78, ptr %ap_80, align 8, !tbaa !1
  %fdp82 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  %t81 = load i64, i64* %fdp82, align 8
  %ap_83 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
  store i64 %t81, ptr %ap_83, align 8, !tbaa !1
  %fdp85 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  %t84 = load i64, i64* %fdp85, align 8
  %ap_86 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
  store i64 %t84, ptr %ap_86, align 8, !tbaa !1
  %fdp88 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  %t87 = load i64, i64* %fdp88, align 8
  %ap_89 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
  store i64 %t87, ptr %ap_89, align 8, !tbaa !1
  %fdp91 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  %t90 = load i64, i64* %fdp91, align 8
  %ap_92 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
  store i64 %t90, ptr %ap_92, align 8, !tbaa !1
  %fdp94 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  %t93 = load i64, i64* %fdp94, align 8
  %ap_95 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
  store i64 %t93, ptr %ap_95, align 8, !tbaa !1
  %fdp97 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  %t96 = load i64, i64* %fdp97, align 8
  %ap_98 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
  store i64 %t96, ptr %ap_98, align 8, !tbaa !1
  %fdp100 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  %t99 = load i64, i64* %fdp100, align 8
  %ap_101 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
  store i64 %t99, ptr %ap_101, align 8, !tbaa !1
  %t102 = add i64 0, %t23
  %ap_103 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
  store i64 %t102, ptr %ap_103, align 8, !tbaa !1
  ret void
}

define internal i1 @pre_fan(ptr noalias nocapture align 8 %state) #10 {
  entry:
  %fdp2 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t1 = load i64, i64* %fdp2, align 8
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t3 = load i64, i64* %fdp4, align 8
%c5 = icmp slt i64 %t1, %t3
  ret i1 %c5
}
define void @init_state(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 {
  entry:
  %ip_0 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
%t1 = add i64 0, 0
  store i64 %t1, ptr %ip_0, align 8
  %ip_1 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
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
  store i64 %t2, ptr %ip_1, align 8
  %ip_2 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
%t17 = add i64 0, 42
  store i64 %t17, ptr %ip_2, align 8
  %ip_3 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
%t19 = add i64 0, 0
  store i64 %t19, ptr %ip_3, align 8
  %ip_4 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
%t21 = add i64 0, 0
  store i64 %t21, ptr %ip_4, align 8
  %ip_5 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
%t23 = add i64 0, 0
  store i64 %t23, ptr %ip_5, align 8
  %ip_6 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
%t25 = add i64 0, 1
  store i64 %t25, ptr %ip_6, align 8
  %ip_7 = getelementptr inbounds %State, ptr %state, i32 0, i32 7
%t27 = add i64 0, 2
  store i64 %t27, ptr %ip_7, align 8
  %ip_8 = getelementptr inbounds %State, ptr %state, i32 0, i32 8
%t29 = add i64 0, 3
  store i64 %t29, ptr %ip_8, align 8
  %ip_9 = getelementptr inbounds %State, ptr %state, i32 0, i32 9
%t31 = add i64 0, 4
  store i64 %t31, ptr %ip_9, align 8
  %ip_10 = getelementptr inbounds %State, ptr %state, i32 0, i32 10
%t33 = add i64 0, 5
  store i64 %t33, ptr %ip_10, align 8
  %ip_11 = getelementptr inbounds %State, ptr %state, i32 0, i32 11
%t35 = add i64 0, 6
  store i64 %t35, ptr %ip_11, align 8
  %ip_12 = getelementptr inbounds %State, ptr %state, i32 0, i32 12
%t37 = add i64 0, 7
  store i64 %t37, ptr %ip_12, align 8
  %ip_13 = getelementptr inbounds %State, ptr %state, i32 0, i32 13
%t39 = add i64 0, 8
  store i64 %t39, ptr %ip_13, align 8
  %ip_14 = getelementptr inbounds %State, ptr %state, i32 0, i32 14
%t41 = add i64 0, 9
  store i64 %t41, ptr %ip_14, align 8
  %ip_15 = getelementptr inbounds %State, ptr %state, i32 0, i32 15
%t43 = add i64 0, 10
  store i64 %t43, ptr %ip_15, align 8
  %ip_16 = getelementptr inbounds %State, ptr %state, i32 0, i32 16
%t45 = add i64 0, 11
  store i64 %t45, ptr %ip_16, align 8
  %ip_17 = getelementptr inbounds %State, ptr %state, i32 0, i32 17
  store i64 0, ptr %ip_17, align 8
  ret void
}


define i32 @main() local_unnamed_addr #9 {
  entry:
  %state_0 = alloca %StateChunk0, align 8
  %state_1 = alloca %StateChunk1, align 8
  %state = alloca %State, align 8
  %ip_46 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
%t48 = add i64 0, 0
  store i64 %t48, ptr %ip_46, align 8
  %ip_49 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
%sp53 = getelementptr inbounds [6 x i8], [6 x i8]* @str.3, i64 0, i64 0
%t52 = ptrtoint i8* %sp53 to i64
  %gsr54 = inttoptr i64 %t52 to ptr
  %gsp55 = bitcast ptr %gsr54 to ptr
  %gdp56 = load i64, ptr %gsp55, align 8
  %gnp57 = inttoptr i64 %gdp56 to ptr
  %gnv58 = call ptr @getenv(ptr %gnp57)
  %gnvl59 = icmp eq ptr %gnv58, null
  br i1 %gnvl59, label %genv_nul60, label %genv_ok61
  genv_nul60:
    br label %genv_af62
  genv_ok61:
  %gav63 = call i64 @atol(ptr %gnv58)
    br label %genv_af62
  genv_af62:
  %t50 = phi i64 [ 0, %genv_nul60 ], [ %gav63, %genv_ok61 ]
  store i64 %t50, ptr %ip_49, align 8
  %ip_64 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
%t66 = add i64 0, 42
  store i64 %t66, ptr %ip_64, align 8
  %ip_67 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
%t69 = add i64 0, 0
  store i64 %t69, ptr %ip_67, align 8
  %ip_70 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
%t72 = add i64 0, 0
  store i64 %t72, ptr %ip_70, align 8
  %ip_73 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
%t75 = add i64 0, 0
  store i64 %t75, ptr %ip_73, align 8
  %ip_76 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
%t78 = add i64 0, 1
  store i64 %t78, ptr %ip_76, align 8
  %ip_79 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
%t81 = add i64 0, 2
  store i64 %t81, ptr %ip_79, align 8
  %ip_82 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
%t84 = add i64 0, 3
  store i64 %t84, ptr %ip_82, align 8
  %ip_85 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
%t87 = add i64 0, 4
  store i64 %t87, ptr %ip_85, align 8
  %ip_88 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
%t90 = add i64 0, 5
  store i64 %t90, ptr %ip_88, align 8
  %ip_91 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
%t93 = add i64 0, 6
  store i64 %t93, ptr %ip_91, align 8
  %ip_94 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
%t96 = add i64 0, 7
  store i64 %t96, ptr %ip_94, align 8
  %ip_97 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
%t99 = add i64 0, 8
  store i64 %t99, ptr %ip_97, align 8
  %ip_100 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 14
%t102 = add i64 0, 9
  store i64 %t102, ptr %ip_100, align 8
  %ip_103 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 0
%t105 = add i64 0, 10
  store i64 %t105, ptr %ip_103, align 8
  %ip_106 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 1
%t108 = add i64 0, 11
  store i64 %t108, ptr %ip_106, align 8
  %ip_109 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 2
  store i64 0, ptr %ip_109, align 8
  %arptr3 = alloca i8*, align 8
  %arend3 = alloca i8*, align 8
  %arbase3 = alloca i8*, align 8
  %arinit3 = call ptr @malloc(i64 65536)
  store i8* %arinit3, ptr %arptr3, align 8
  store i8* %arinit3, ptr %arbase3, align 8
  %arieu3 = getelementptr i8, ptr %arinit3, i64 65536
  store i8* %arieu3, ptr %arend3, align 8
  %gt_110 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
  %cnt_bound_46 = load i64, ptr %gt_110, align 8
  br label %pre_phi
pre_phi:
  %lv_111 = alloca i64, align 8
  %init_cnt_112 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
  %init_N_113 = load i64, ptr %init_cnt_112, align 8, !tbaa !1
  %init_cnt_114 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %init_checksum_115 = load i64, ptr %init_cnt_114, align 8, !tbaa !1
  %init_cnt_116 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 2
  %init_cycle_count_117 = load i64, ptr %init_cnt_116, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_118 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %init_max_flips_119 = load i64, ptr %init_cnt_118, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_120 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %init_p0_121 = load i64, ptr %init_cnt_120, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_122 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %init_p1_123 = load i64, ptr %init_cnt_122, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_124 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 0
  %init_p10_125 = load i64, ptr %init_cnt_124, align 8, !tbaa !1
  %init_cnt_126 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 1
  %init_p11_127 = load i64, ptr %init_cnt_126, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_128 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
  %init_p2_129 = load i64, ptr %init_cnt_128, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_130 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
  %init_p3_131 = load i64, ptr %init_cnt_130, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_132 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
  %init_p4_133 = load i64, ptr %init_cnt_132, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_134 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
  %init_p5_135 = load i64, ptr %init_cnt_134, align 8, !tbaa !1
  %init_cnt_136 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
  %init_p6_137 = load i64, ptr %init_cnt_136, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_138 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
  %init_p7_139 = load i64, ptr %init_cnt_138, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_140 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
  %init_p8_141 = load i64, ptr %init_cnt_140, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_142 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 14
  %init_p9_143 = load i64, ptr %init_cnt_142, align 8, !tbaa !1, !invariant.load !{}
  %init_cnt_144 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %init_seed_145 = load i64, ptr %init_cnt_144, align 8, !tbaa !1
  %init_cnt_146 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %init_count_147 = load i64, ptr %init_cnt_146, align 8
  br label %loop_hdr
loop_hdr:
  %pi_cnt_148 = phi i64 [ %init_count_147, %pre_phi ], [ %pn_cnt_148, %latch ]
  %phi_p7 = phi i64 [ %init_p7_139, %pre_phi ], [ %be_p7, %latch ]
  %phi_max_flips = phi i64 [ %init_max_flips_119, %pre_phi ], [ %be_max_flips, %latch ]
  %phi_cycle_count = phi i64 [ %init_cycle_count_117, %pre_phi ], [ %be_cycle_count, %latch ]
  %phi_p5 = phi i64 [ %init_p5_135, %pre_phi ], [ %be_p5, %latch ]
  %phi_checksum = phi i64 [ %init_checksum_115, %pre_phi ], [ %be_checksum, %latch ]
  %phi_p0 = phi i64 [ %init_p0_121, %pre_phi ], [ %be_p0, %latch ]
  %phi_p1 = phi i64 [ %init_p1_123, %pre_phi ], [ %be_p1, %latch ]
  %phi_p6 = phi i64 [ %init_p6_137, %pre_phi ], [ %be_p6, %latch ]
  %phi_p2 = phi i64 [ %init_p2_129, %pre_phi ], [ %be_p2, %latch ]
  %phi_p4 = phi i64 [ %init_p4_133, %pre_phi ], [ %be_p4, %latch ]
  %phi_seed = phi i64 [ %init_seed_145, %pre_phi ], [ %be_seed, %latch ]
  %phi_p10 = phi i64 [ %init_p10_125, %pre_phi ], [ %be_p10, %latch ]
  %phi_p9 = phi i64 [ %init_p9_143, %pre_phi ], [ %be_p9, %latch ]
  %phi_p11 = phi i64 [ %init_p11_127, %pre_phi ], [ %be_p11, %latch ]
  %phi_p3 = phi i64 [ %init_p3_131, %pre_phi ], [ %be_p3, %latch ]
  %phi_p8 = phi i64 [ %init_p8_141, %pre_phi ], [ %be_p8, %latch ]
  %phi_N = phi i64 [ %init_N_113, %pre_phi ], [ %be_N, %latch ]
  %phi_count = phi i64 [ %init_count_147, %pre_phi ], [ %be_count, %latch ]
  %cmp_hdr_149 = icmp slt i64 %pi_cnt_148, %cnt_bound_46
  br i1 %cmp_hdr_149, label %body, label %commit
body:
  %t153 = add i64 0, %phi_seed
  %il155 = load i64, i64* @IA, align 8
  %t154 = add i64 0, %il155
%t156 = mul nsw i64 %t153, %t154
  %il158 = load i64, i64* @IC, align 8
  %t157 = add i64 0, %il158
%t159 = add nsw i64 %t156, %t157
  %il161 = load i64, i64* @IM, align 8
  %t160 = add i64 0, %il161
%t162 = srem i64 %t159, %t160
  ; let ns = %t162
  %t163 = add i64 0, %phi_p0
  ; let saved = %t163
  %t165 = add i64 0, %phi_checksum
  %t167 = add i64 0, %t163
%t169 = add i64 0, 13
%t170 = srem i64 %t167, %t169
%t171 = add nsw i64 %t165, %t170
  ; let nchecksum = %t171
  %t173 = add i64 0, %phi_max_flips
  %t175 = add i64 0, %t171
%t177 = add i64 0, 17
%t178 = srem i64 %t175, %t177
%t179 = add nsw i64 %t173, %t178
  ; let nmax = %t179
  %t180 = add i64 0, %t162
  %ap_181 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  store i64 %t180, ptr %ap_181, align 8, !tbaa !1
  %t182 = add i64 0, %phi_p1
  %ap_183 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %t184 = add i64 0, %phi_p2
  %ap_185 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %t186 = add i64 0, %phi_p3
  %ap_187 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
  %t188 = add i64 0, %phi_p4
  %ap_189 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
  %t190 = add i64 0, %phi_p5
  %ap_191 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
  %t192 = add i64 0, %phi_p6
  %ap_193 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
  %t194 = add i64 0, %phi_p7
  %ap_195 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
  %t196 = add i64 0, %phi_p8
  %ap_197 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
  %t198 = add i64 0, %phi_p9
  %ap_199 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
  %t200 = add i64 0, %phi_p10
  %ap_201 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 14
  %t202 = add i64 0, %phi_p11
  %ap_203 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 0
  %t204 = add i64 0, %t163
  %ap_205 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 1
  %t206 = add i64 0, %t171
  %ap_207 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  store i64 %t206, ptr %ap_207, align 8, !tbaa !1
  %t208 = add i64 0, %t179
  %ap_209 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  store i64 %t208, ptr %ap_209, align 8, !tbaa !1
  %t211 = add i64 0, %phi_count
%t213 = add i64 0, 1
%t214 = add nsw i64 %t211, %t213
  %ap_215 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  store i64 %t214, ptr %ap_215, align 8, !tbaa !1
  %full_next_217 = add i64 %phi_count, 4
  %full_chk_216 = icmp sle i64 %full_next_217, %cnt_bound_46
  br i1 %full_chk_216, label %rot_full, label %rot_cold
rot_full:
  %rr_218 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %rld_219 = load i64, ptr %rr_218, align 8
  %rr_220 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %rld_221 = load i64, ptr %rr_220, align 8
  %rr_222 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %rld_223 = load i64, ptr %rr_222, align 8
  %rr_224 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %rld_225 = load i64, ptr %rr_224, align 8
  %rc_226 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %rlc_227 = load i64, ptr %rc_226, align 8
  %t231 = add i64 0, %rld_223
  %il233 = load i64, i64* @IA, align 8
  %t232 = add i64 0, %il233
%t234 = mul nsw i64 %t231, %t232
  %il236 = load i64, i64* @IC, align 8
  %t235 = add i64 0, %il236
%t237 = add nsw i64 %t234, %t235
  %il239 = load i64, i64* @IM, align 8
  %t238 = add i64 0, %il239
%t240 = srem i64 %t237, %t238
  ; let ns = %t240
  %t241 = add i64 0, %t182
  ; let saved = %t241
  %t243 = add i64 0, %rld_219
  %t245 = add i64 0, %t241
%t247 = add i64 0, 13
%t248 = srem i64 %t245, %t247
%t249 = add nsw i64 %t243, %t248
  ; let nchecksum = %t249
  %t251 = add i64 0, %rld_221
  %t253 = add i64 0, %t249
%t255 = add i64 0, 17
%t256 = srem i64 %t253, %t255
%t257 = add nsw i64 %t251, %t256
  ; let nmax = %t257
  %t258 = add i64 0, %t240
  %ap_259 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  store i64 %t258, ptr %ap_259, align 8, !tbaa !1
  %t260 = add i64 0, %t184
  %ap_261 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %t262 = add i64 0, %t186
  %ap_263 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %t264 = add i64 0, %t188
  %ap_265 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
  %t266 = add i64 0, %t190
  %ap_267 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
  %t268 = add i64 0, %t192
  %ap_269 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
  %t270 = add i64 0, %t194
  %ap_271 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
  %t272 = add i64 0, %t196
  %ap_273 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
  %t274 = add i64 0, %t198
  %ap_275 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
  %t276 = add i64 0, %t200
  %ap_277 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
  %t278 = add i64 0, %t202
  %ap_279 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 14
  %t280 = add i64 0, %t204
  %ap_281 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 0
  %t282 = add i64 0, %t241
  %ap_283 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 1
  %t284 = add i64 0, %t249
  %ap_285 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  store i64 %t284, ptr %ap_285, align 8, !tbaa !1
  %t286 = add i64 0, %t257
  %ap_287 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  store i64 %t286, ptr %ap_287, align 8, !tbaa !1
  %t289 = add i64 0, %rlc_227
%t291 = add i64 0, 1
%t292 = add nsw i64 %t289, %t291
  %ap_293 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  store i64 %t292, ptr %ap_293, align 8, !tbaa !1
  %rr_294 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %rld_295 = load i64, ptr %rr_294, align 8
  %rr_296 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %rld_297 = load i64, ptr %rr_296, align 8
  %rr_298 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %rld_299 = load i64, ptr %rr_298, align 8
  %rr_300 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %rld_301 = load i64, ptr %rr_300, align 8
  %rc_302 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %rlc_303 = load i64, ptr %rc_302, align 8
  %t307 = add i64 0, %rld_299
  %il309 = load i64, i64* @IA, align 8
  %t308 = add i64 0, %il309
%t310 = mul nsw i64 %t307, %t308
  %il312 = load i64, i64* @IC, align 8
  %t311 = add i64 0, %il312
%t313 = add nsw i64 %t310, %t311
  %il315 = load i64, i64* @IM, align 8
  %t314 = add i64 0, %il315
%t316 = srem i64 %t313, %t314
  ; let ns = %t316
  %t317 = add i64 0, %t260
  ; let saved = %t317
  %t319 = add i64 0, %rld_295
  %t321 = add i64 0, %t317
%t323 = add i64 0, 13
%t324 = srem i64 %t321, %t323
%t325 = add nsw i64 %t319, %t324
  ; let nchecksum = %t325
  %t327 = add i64 0, %rld_297
  %t329 = add i64 0, %t325
%t331 = add i64 0, 17
%t332 = srem i64 %t329, %t331
%t333 = add nsw i64 %t327, %t332
  ; let nmax = %t333
  %t334 = add i64 0, %t316
  %ap_335 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  store i64 %t334, ptr %ap_335, align 8, !tbaa !1
  %t336 = add i64 0, %t262
  %ap_337 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %t338 = add i64 0, %t264
  %ap_339 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %t340 = add i64 0, %t266
  %ap_341 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
  %t342 = add i64 0, %t268
  %ap_343 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
  %t344 = add i64 0, %t270
  %ap_345 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
  %t346 = add i64 0, %t272
  %ap_347 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
  %t348 = add i64 0, %t274
  %ap_349 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
  %t350 = add i64 0, %t276
  %ap_351 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
  %t352 = add i64 0, %t278
  %ap_353 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
  %t354 = add i64 0, %t280
  %ap_355 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 14
  %t356 = add i64 0, %t282
  %ap_357 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 0
  %t358 = add i64 0, %t317
  %ap_359 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 1
  %t360 = add i64 0, %t325
  %ap_361 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  store i64 %t360, ptr %ap_361, align 8, !tbaa !1
  %t362 = add i64 0, %t333
  %ap_363 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  store i64 %t362, ptr %ap_363, align 8, !tbaa !1
  %t365 = add i64 0, %rlc_303
%t367 = add i64 0, 1
%t368 = add nsw i64 %t365, %t367
  %ap_369 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  store i64 %t368, ptr %ap_369, align 8, !tbaa !1
  %rr_370 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %rld_371 = load i64, ptr %rr_370, align 8
  %rr_372 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %rld_373 = load i64, ptr %rr_372, align 8
  %rr_374 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %rld_375 = load i64, ptr %rr_374, align 8
  %rr_376 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %rld_377 = load i64, ptr %rr_376, align 8
  %rc_378 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %rlc_379 = load i64, ptr %rc_378, align 8
  %t383 = add i64 0, %rld_375
  %il385 = load i64, i64* @IA, align 8
  %t384 = add i64 0, %il385
%t386 = mul nsw i64 %t383, %t384
  %il388 = load i64, i64* @IC, align 8
  %t387 = add i64 0, %il388
%t389 = add nsw i64 %t386, %t387
  %il391 = load i64, i64* @IM, align 8
  %t390 = add i64 0, %il391
%t392 = srem i64 %t389, %t390
  ; let ns = %t392
  %t393 = add i64 0, %t336
  ; let saved = %t393
  %t395 = add i64 0, %rld_371
  %t397 = add i64 0, %t393
%t399 = add i64 0, 13
%t400 = srem i64 %t397, %t399
%t401 = add nsw i64 %t395, %t400
  ; let nchecksum = %t401
  %t403 = add i64 0, %rld_373
  %t405 = add i64 0, %t401
%t407 = add i64 0, 17
%t408 = srem i64 %t405, %t407
%t409 = add nsw i64 %t403, %t408
  ; let nmax = %t409
  %t410 = add i64 0, %t392
  %ap_411 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  store i64 %t410, ptr %ap_411, align 8, !tbaa !1
  %t412 = add i64 0, %t338
  %ap_413 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %t414 = add i64 0, %t340
  %ap_415 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %t416 = add i64 0, %t342
  %ap_417 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
  %t418 = add i64 0, %t344
  %ap_419 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
  %t420 = add i64 0, %t346
  %ap_421 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
  %t422 = add i64 0, %t348
  %ap_423 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
  %t424 = add i64 0, %t350
  %ap_425 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
  %t426 = add i64 0, %t352
  %ap_427 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
  %t428 = add i64 0, %t354
  %ap_429 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
  %t430 = add i64 0, %t356
  %ap_431 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 14
  %t432 = add i64 0, %t358
  %ap_433 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 0
  %t434 = add i64 0, %t393
  %ap_435 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 1
  %t436 = add i64 0, %t401
  %ap_437 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  store i64 %t436, ptr %ap_437, align 8, !tbaa !1
  %t438 = add i64 0, %t409
  %ap_439 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  store i64 %t438, ptr %ap_439, align 8, !tbaa !1
  %t441 = add i64 0, %rlc_379
%t443 = add i64 0, 1
%t444 = add nsw i64 %t441, %t443
  %ap_445 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  store i64 %t444, ptr %ap_445, align 8, !tbaa !1
  br label %latch
rot_cold:
  %rr_446 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %rld_447 = load i64, ptr %rr_446, align 8
  %rr_448 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %rld_449 = load i64, ptr %rr_448, align 8
  %rr_450 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %rld_451 = load i64, ptr %rr_450, align 8
  %rr_452 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %rld_453 = load i64, ptr %rr_452, align 8
  %rc_454 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %rlc_455 = load i64, ptr %rc_454, align 8
  %ro_chk_456 = icmp sge i64 %rlc_455, %cnt_bound_46
  br i1 %ro_chk_456, label %latch, label %body_rot1
body_rot1:
  %t460 = add i64 0, %rld_451
  %il462 = load i64, i64* @IA, align 8
  %t461 = add i64 0, %il462
%t463 = mul nsw i64 %t460, %t461
  %il465 = load i64, i64* @IC, align 8
  %t464 = add i64 0, %il465
%t466 = add nsw i64 %t463, %t464
  %il468 = load i64, i64* @IM, align 8
  %t467 = add i64 0, %il468
%t469 = srem i64 %t466, %t467
  ; let ns = %t469
  %t470 = add i64 0, %t182
  ; let saved = %t470
  %t472 = add i64 0, %rld_447
  %t474 = add i64 0, %t470
%t476 = add i64 0, 13
%t477 = srem i64 %t474, %t476
%t478 = add nsw i64 %t472, %t477
  ; let nchecksum = %t478
  %t480 = add i64 0, %rld_449
  %t482 = add i64 0, %t478
%t484 = add i64 0, 17
%t485 = srem i64 %t482, %t484
%t486 = add nsw i64 %t480, %t485
  ; let nmax = %t486
  %t487 = add i64 0, %t469
  %ap_488 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  store i64 %t487, ptr %ap_488, align 8, !tbaa !1
  %t489 = add i64 0, %t184
  %ap_490 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %t491 = add i64 0, %t186
  %ap_492 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %t493 = add i64 0, %t188
  %ap_494 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
  %t495 = add i64 0, %t190
  %ap_496 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
  %t497 = add i64 0, %t192
  %ap_498 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
  %t499 = add i64 0, %t194
  %ap_500 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
  %t501 = add i64 0, %t196
  %ap_502 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
  %t503 = add i64 0, %t198
  %ap_504 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
  %t505 = add i64 0, %t200
  %ap_506 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
  %t507 = add i64 0, %t202
  %ap_508 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 14
  %t509 = add i64 0, %t204
  %ap_510 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 0
  %t511 = add i64 0, %t470
  %ap_512 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 1
  %t513 = add i64 0, %t478
  %ap_514 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  store i64 %t513, ptr %ap_514, align 8, !tbaa !1
  %t515 = add i64 0, %t486
  %ap_516 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  store i64 %t515, ptr %ap_516, align 8, !tbaa !1
  %t518 = add i64 0, %rlc_455
%t520 = add i64 0, 1
%t521 = add nsw i64 %t518, %t520
  %ap_522 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  store i64 %t521, ptr %ap_522, align 8, !tbaa !1
  %rr_523 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %rld_524 = load i64, ptr %rr_523, align 8
  %rr_525 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %rld_526 = load i64, ptr %rr_525, align 8
  %rr_527 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %rld_528 = load i64, ptr %rr_527, align 8
  %rr_529 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %rld_530 = load i64, ptr %rr_529, align 8
  %rc_531 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %rlc_532 = load i64, ptr %rc_531, align 8
  %ro_chk_533 = icmp sge i64 %rlc_532, %cnt_bound_46
  br i1 %ro_chk_533, label %latch, label %body_rot2
body_rot2:
  %t537 = add i64 0, %rld_528
  %il539 = load i64, i64* @IA, align 8
  %t538 = add i64 0, %il539
%t540 = mul nsw i64 %t537, %t538
  %il542 = load i64, i64* @IC, align 8
  %t541 = add i64 0, %il542
%t543 = add nsw i64 %t540, %t541
  %il545 = load i64, i64* @IM, align 8
  %t544 = add i64 0, %il545
%t546 = srem i64 %t543, %t544
  ; let ns = %t546
  %t547 = add i64 0, %t489
  ; let saved = %t547
  %t549 = add i64 0, %rld_524
  %t551 = add i64 0, %t547
%t553 = add i64 0, 13
%t554 = srem i64 %t551, %t553
%t555 = add nsw i64 %t549, %t554
  ; let nchecksum = %t555
  %t557 = add i64 0, %rld_526
  %t559 = add i64 0, %t555
%t561 = add i64 0, 17
%t562 = srem i64 %t559, %t561
%t563 = add nsw i64 %t557, %t562
  ; let nmax = %t563
  %t564 = add i64 0, %t546
  %ap_565 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  store i64 %t564, ptr %ap_565, align 8, !tbaa !1
  %t566 = add i64 0, %t491
  %ap_567 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %t568 = add i64 0, %t493
  %ap_569 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %t570 = add i64 0, %t495
  %ap_571 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
  %t572 = add i64 0, %t497
  %ap_573 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
  %t574 = add i64 0, %t499
  %ap_575 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
  %t576 = add i64 0, %t501
  %ap_577 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
  %t578 = add i64 0, %t503
  %ap_579 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
  %t580 = add i64 0, %t505
  %ap_581 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
  %t582 = add i64 0, %t507
  %ap_583 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
  %t584 = add i64 0, %t509
  %ap_585 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 14
  %t586 = add i64 0, %t511
  %ap_587 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 0
  %t588 = add i64 0, %t547
  %ap_589 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 1
  %t590 = add i64 0, %t555
  %ap_591 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  store i64 %t590, ptr %ap_591, align 8, !tbaa !1
  %t592 = add i64 0, %t563
  %ap_593 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  store i64 %t592, ptr %ap_593, align 8, !tbaa !1
  %t595 = add i64 0, %rlc_532
%t597 = add i64 0, 1
%t598 = add nsw i64 %t595, %t597
  %ap_599 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  store i64 %t598, ptr %ap_599, align 8, !tbaa !1
  %rr_600 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %rld_601 = load i64, ptr %rr_600, align 8
  %rr_602 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %rld_603 = load i64, ptr %rr_602, align 8
  %rr_604 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %rld_605 = load i64, ptr %rr_604, align 8
  %rr_606 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %rld_607 = load i64, ptr %rr_606, align 8
  %rc_608 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %rlc_609 = load i64, ptr %rc_608, align 8
  %ro_chk_610 = icmp sge i64 %rlc_609, %cnt_bound_46
  br i1 %ro_chk_610, label %latch, label %body_rot3
body_rot3:
  %t614 = add i64 0, %rld_605
  %il616 = load i64, i64* @IA, align 8
  %t615 = add i64 0, %il616
%t617 = mul nsw i64 %t614, %t615
  %il619 = load i64, i64* @IC, align 8
  %t618 = add i64 0, %il619
%t620 = add nsw i64 %t617, %t618
  %il622 = load i64, i64* @IM, align 8
  %t621 = add i64 0, %il622
%t623 = srem i64 %t620, %t621
  ; let ns = %t623
  %t624 = add i64 0, %t566
  ; let saved = %t624
  %t626 = add i64 0, %rld_601
  %t628 = add i64 0, %t624
%t630 = add i64 0, 13
%t631 = srem i64 %t628, %t630
%t632 = add nsw i64 %t626, %t631
  ; let nchecksum = %t632
  %t634 = add i64 0, %rld_603
  %t636 = add i64 0, %t632
%t638 = add i64 0, 17
%t639 = srem i64 %t636, %t638
%t640 = add nsw i64 %t634, %t639
  ; let nmax = %t640
  %t641 = add i64 0, %t623
  %ap_642 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  store i64 %t641, ptr %ap_642, align 8, !tbaa !1
  %t643 = add i64 0, %t568
  %ap_644 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %t645 = add i64 0, %t570
  %ap_646 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %t647 = add i64 0, %t572
  %ap_648 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 7
  %t649 = add i64 0, %t574
  %ap_650 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 8
  %t651 = add i64 0, %t576
  %ap_652 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 9
  %t653 = add i64 0, %t578
  %ap_654 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 10
  %t655 = add i64 0, %t580
  %ap_656 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 11
  %t657 = add i64 0, %t582
  %ap_658 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 12
  %t659 = add i64 0, %t584
  %ap_660 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 13
  %t661 = add i64 0, %t586
  %ap_662 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 14
  %t663 = add i64 0, %t588
  %ap_664 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 0
  %t665 = add i64 0, %t624
  %ap_666 = getelementptr inbounds %StateChunk1, ptr %state_1, i32 0, i32 1
  %t667 = add i64 0, %t632
  %ap_668 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  store i64 %t667, ptr %ap_668, align 8, !tbaa !1
  %t669 = add i64 0, %t640
  %ap_670 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  store i64 %t669, ptr %ap_670, align 8, !tbaa !1
  %t672 = add i64 0, %rlc_609
%t674 = add i64 0, 1
%t675 = add nsw i64 %t672, %t674
  %ap_676 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  store i64 %t675, ptr %ap_676, align 8, !tbaa !1
  br label %latch
latch:
  %pn_cnt_148 = add i64 %pi_cnt_148, 4
  %be_N = add i64 0, %phi_N
  %be_677 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %be_checksum = load i64, ptr %be_677, align 8
  %be_cycle_count = add i64 0, %phi_cycle_count
  %be_678 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %be_max_flips = load i64, ptr %be_678, align 8
  %be_p0 = add i64 0, %phi_p4
  %be_p1 = add i64 0, %phi_p5
  %be_p10 = add i64 0, %phi_p2
  %be_p11 = add i64 0, %phi_p3
  %be_p2 = add i64 0, %phi_p6
  %be_p3 = add i64 0, %phi_p7
  %be_p4 = add i64 0, %phi_p8
  %be_p5 = add i64 0, %phi_p9
  %be_p6 = add i64 0, %phi_p10
  %be_p7 = add i64 0, %phi_p11
  %be_p8 = add i64 0, %phi_p0
  %be_p9 = add i64 0, %phi_p1
  %be_679 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %be_seed = load i64, ptr %be_679, align 8
  %be_count = add i64 0, %pn_cnt_148
  br label %loop_hdr, !llvm.loop !100
commit:
  store i64 %phi_checksum, ptr %lv_111, align 8
  br label %done
done:
  %arr4 = load ptr, ptr %arbase3, align 8
  store i8* %arr4, ptr %arptr3, align 8
  %lv_680 = load i64, ptr %lv_111, align 8
  %t682 = add i64 0, %lv_680
  %pso683 = load volatile ptr, ptr @stdout
  %pfi684 = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0
  %ppi685 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso683, ptr %pfi684, i64 %t682)
  %t681 = zext i32 %ppi685 to i64
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

!0 = !{!"Brief"}
!1 = !{!"Int", !0}
!2 = !{!"Char", !0}
!3 = !{!"Float", !0}
!4 = !{!"Bool", !0}
!5 = !{!"String", !0}
!99 = distinct !{} ; StateAliasScope
