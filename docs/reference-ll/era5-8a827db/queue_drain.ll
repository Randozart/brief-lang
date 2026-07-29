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
%StateChunk0 = type { i64, i64, i64, i64, i64, i64, i64 }
%State = type { i64, i64, i64, i64, i64, i64, i64 }
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

define void @work(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp2 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t1 = load i64, i64* %fdp2, align 1
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 1
%c5 = icmp slt i64 %t1, %t3
  %pi6 = and i1 %c5, true
  br i1 %pi6, label %ps8, label %pp7
  pp7:
    unreachable
  ps8:
  call void @llvm.assume(i1 %pi6)
  %rbhg10 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %rbhv11 = load i64, ptr %rbhg10, align 8
  %rbtg12 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %rbtv13 = load i64, ptr %rbtg12, align 8
  %rbmk14 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %rbmv15 = load i64, ptr %rbmk14, align 8
  %rbdg16 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %rbdv17 = load i64, ptr %rbdg16, align 8
  %rbdp18 = inttoptr i64 %rbdv17 to ptr
  %rbem19 = icmp eq i64 %rbhv11, %rbtv13
  %rbsl20 = getelementptr i64, ptr %rbdp18, i64 %rbhv11
  %rbvr21 = load i64, ptr %rbsl20, align 8
  %rbhn22 = add i64 %rbhv11, 1
  %rbhw23 = and i64 %rbhn22, %rbmv15
  %rbnw24 = select i1 %rbem19, i64 %rbhv11, i64 %rbhw23
  store i64 %rbnw24, i64* %rbhg10, align 8
  %rbz25 = add i64 0, 0
  %rbpo26 = select i1 %rbem19, i64 %rbz25, i64 %rbvr21
  %fdp29 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t28 = load i64, i64* %fdp29, align 1
  %rbtg30 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %rbtv31 = load i64, ptr %rbtg30, align 8
  %rbmk32 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %rbmv33 = load i64, ptr %rbmk32, align 8
  %rbdg34 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %rbdv35 = load i64, ptr %rbdg34, align 8
  %rbdp36 = inttoptr i64 %rbdv35 to ptr
  %rbsl37 = getelementptr i64, ptr %rbdp36, i64 %rbtv31
  store i64 %t28, i64* %rbsl37, align 8
  %rbtn38 = add i64 %rbtv31, 1
  %rbnt39 = and i64 %rbtn38, %rbmv33
  store i64 %rbnt39, i64* %rbtg30, align 8
  %fdp42 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t41 = load i64, i64* %fdp42, align 1
%t44 = add i64 0, 1
%t45 = add nsw i64 %t41, %t44
  %ts_46 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 %t45, ptr %ts_46, align 1, !tbaa !1
  %fdp50 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t49 = load i64, i64* %fdp50, align 1
%t52 = add i64 0, 5000000
%t53 = srem i64 %t49, %t52
%t55 = add i64 0, 0
%c56 = icmp eq i64 %t53, %t55
  br i1 %c56, label %g57_t, label %g57_e
  g57_t:
    %fdp60 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
    %t59 = load i64, i64* %fdp60, align 1
    %pso61 = load volatile ptr, ptr @stdout
    %pfi62 = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0
    %ppi63 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso61, ptr %pfi62, i64 %t59)
    %t58 = zext i32 %ppi63 to i64
    br label %g57_tx
  g57_tx:
    br label %g57_e
  g57_e:
  ret void
}

define internal i1 @pre_work(ptr noalias nocapture align 8 %state) #10 {
  entry:
  %fdp2 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t1 = load i64, i64* %fdp2, align 1
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 1
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
  store i64 %t0, ptr %ip_0, align 1
  %ip_1 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
%t15 = add i64 0, 0
  store i64 %t15, ptr %ip_1, align 1
  %ip_2 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 0, ptr %ip_2, align 1
  %ip_3 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  store i64 0, ptr %ip_3, align 1
  %ip_4 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  store i64 0, ptr %ip_4, align 1
  %ip_5 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  store i64 0, ptr %ip_5, align 1
  %ip_6 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  store i64 0, ptr %ip_6, align 1
  ret void
}


define i32 @main() local_unnamed_addr #9 {
  entry:
  %state_0 = alloca %StateChunk0, align 8
  %state = alloca %State, align 8
  %ip_16 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
%sp20 = getelementptr inbounds [6 x i8], [6 x i8]* @str.3, i64 0, i64 0
%t19 = ptrtoint i8* %sp20 to i64
  %gsr21 = inttoptr i64 %t19 to ptr
  %gsp22 = bitcast ptr %gsr21 to ptr
  %gdp23 = load i64, ptr %gsp22, align 8
  %gnp24 = inttoptr i64 %gdp23 to ptr
  %gnv25 = call ptr @getenv(ptr %gnp24)
  %gnvl26 = icmp eq ptr %gnv25, null
  br i1 %gnvl26, label %genv_nul27, label %genv_ok28
  genv_nul27:
    br label %genv_af29
  genv_ok28:
  %gav30 = call i64 @atol(ptr %gnv25)
    br label %genv_af29
  genv_af29:
  %t17 = phi i64 [ 0, %genv_nul27 ], [ %gav30, %genv_ok28 ]
  store i64 %t17, ptr %ip_16, align 1
  %ip_31 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
%t33 = add i64 0, 0
  store i64 %t33, ptr %ip_31, align 1
  %ip_34 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  store i64 0, ptr %ip_34, align 1
  %ip_35 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  store i64 0, ptr %ip_35, align 1
  %ip_36 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  store i64 0, ptr %ip_36, align 1
  %ip_37 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  store i64 0, ptr %ip_37, align 1
  %ip_38 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  store i64 0, ptr %ip_38, align 1
  %arptr3 = alloca i8*, align 8
  %arend3 = alloca i8*, align 8
  %arbase3 = alloca i8*, align 8
  %arinit3 = call ptr @malloc(i64 65536)
  store i8* %arinit3, ptr %arptr3, align 8
  store i8* %arinit3, ptr %arbase3, align 8
  %arieu3 = getelementptr i8, ptr %arinit3, i64 65536
  store i8* %arieu3, ptr %arend3, align 8
  %gt_39 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %cnt_bound_16 = load i64, ptr %gt_39, align 8
  br label %pre_phi
pre_phi:
  %init_cnt_40 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %init_N_41 = load i64, ptr %init_cnt_40, align 1, !tbaa !1
  %init_cnt_42 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 6
  %init_cycle_count_43 = load i64, ptr %init_cnt_42, align 1, !tbaa !1, !invariant.load !{}
  %init_cnt_44 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %init_queue_data_45 = load i64, ptr %init_cnt_44, align 1, !tbaa !1, !invariant.load !{}
  %init_cnt_46 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 3
  %init_queue_head_47 = load i64, ptr %init_cnt_46, align 1, !tbaa !1, !invariant.load !{}
  %init_cnt_48 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 5
  %init_queue_mask_49 = load i64, ptr %init_cnt_48, align 1, !tbaa !1, !invariant.load !{}
  %init_cnt_50 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 4
  %init_queue_tail_51 = load i64, ptr %init_cnt_50, align 1, !tbaa !1, !invariant.load !{}
  %init_cnt_52 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
  %init_count_53 = load i64, ptr %init_cnt_52, align 8
  br label %loop_hdr
loop_hdr:
  %pi_cnt_54 = phi i64 [ %init_count_53, %pre_phi ], [ %pn_cnt_54, %latch ]
  %phi_queue_tail = phi i64 [ %init_queue_tail_51, %pre_phi ], [ %be_queue_tail, %latch ]
  %phi_N = phi i64 [ %init_N_41, %pre_phi ], [ %be_N, %latch ]
  %phi_cycle_count = phi i64 [ %init_cycle_count_43, %pre_phi ], [ %be_cycle_count, %latch ]
  %phi_queue_mask = phi i64 [ %init_queue_mask_49, %pre_phi ], [ %be_queue_mask, %latch ]
  %phi_queue_data = phi i64 [ %init_queue_data_45, %pre_phi ], [ %be_queue_data, %latch ]
  %phi_queue_head = phi i64 [ %init_queue_head_47, %pre_phi ], [ %be_queue_head, %latch ]
  %phi_count = phi i64 [ %init_count_53, %pre_phi ], [ %be_count, %latch ]
  %cmp_hdr_55 = icmp slt i64 %pi_cnt_54, %cnt_bound_16
  br i1 %cmp_hdr_55, label %body, label %done
body:
  %rbhg57 = getelementptr inbounds %State, ptr %state, i32 0, i32 4
  %rbhv58 = load i64, ptr %rbhg57, align 8
  %rbtg59 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %rbtv60 = load i64, ptr %rbtg59, align 8
  %rbmk61 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %rbmv62 = load i64, ptr %rbmk61, align 8
  %rbdg63 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %rbdv64 = load i64, ptr %rbdg63, align 8
  %rbdp65 = inttoptr i64 %rbdv64 to ptr
  %rbem66 = icmp eq i64 %rbhv58, %rbtv60
  %rbsl67 = getelementptr i64, ptr %rbdp65, i64 %rbhv58
  %rbvr68 = load i64, ptr %rbsl67, align 8
  %rbhn69 = add i64 %rbhv58, 1
  %rbhw70 = and i64 %rbhn69, %rbmv62
  %rbnw71 = select i1 %rbem66, i64 %rbhv58, i64 %rbhw70
  store i64 %rbnw71, i64* %rbhg57, align 8
  %rbz72 = add i64 0, 0
  %rbpo73 = select i1 %rbem66, i64 %rbz72, i64 %rbvr68
  %t75 = add i64 0, %phi_count
  %rbtg76 = getelementptr inbounds %State, ptr %state, i32 0, i32 5
  %rbtv77 = load i64, ptr %rbtg76, align 8
  %rbmk78 = getelementptr inbounds %State, ptr %state, i32 0, i32 6
  %rbmv79 = load i64, ptr %rbmk78, align 8
  %rbdg80 = getelementptr inbounds %State, ptr %state, i32 0, i32 3
  %rbdv81 = load i64, ptr %rbdg80, align 8
  %rbdp82 = inttoptr i64 %rbdv81 to ptr
  %rbsl83 = getelementptr i64, ptr %rbdp82, i64 %rbtv77
  store i64 %t75, i64* %rbsl83, align 8
  %rbtn84 = add i64 %rbtv77, 1
  %rbnt85 = and i64 %rbtn84, %rbmv79
  store i64 %rbnt85, i64* %rbtg76, align 8
  %t87 = add i64 0, %phi_count
%t89 = add i64 0, 1
%t90 = add nsw i64 %t87, %t89
  %ts_91 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
  store i64 %t90, ptr %ts_91, align 1, !tbaa !1
  %t94 = add i64 0, %t90
%t96 = add i64 0, 5000000
%t97 = srem i64 %t94, %t96
%t99 = add i64 0, 0
%c100 = icmp eq i64 %t97, %t99
  br i1 %c100, label %g101_t, label %g101_e
  g101_t:
    %t103 = add i64 0, %t90
    %pso104 = load volatile ptr, ptr @stdout
    %pfi105 = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0
    %ppi106 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso104, ptr %pfi105, i64 %t103)
    %t102 = zext i32 %ppi106 to i64
    br label %g101_tx
  g101_tx:
    br label %g101_e
  g101_e:
  br label %latch
latch:
  %pn_cnt_54 = add i64 %pi_cnt_54, 1
  %be_N = add i64 0, %phi_N
  %be_cycle_count = add i64 0, %phi_cycle_count
  %be_queue_data = add i64 0, %phi_queue_data
  %be_queue_head = add i64 0, %phi_queue_head
  %be_queue_mask = add i64 0, %phi_queue_mask
  %be_queue_tail = add i64 0, %phi_queue_tail
  %be_count = add i64 0, %pn_cnt_54
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

!0 = !{!"Brief"}
!1 = !{!"Int", !0}
!2 = !{!"Char", !0}
!3 = !{!"Float", !0}
!4 = !{!"Bool", !0}
!5 = !{!"String", !0}
!99 = distinct !{} ; StateAliasScope
