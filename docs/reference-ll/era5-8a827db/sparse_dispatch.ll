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
%StateChunk0 = type { i64, i64, i64 }
%State = type { i64, i64, i64 }
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

define void @ping(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 8
  %fdp6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t5 = load i64, i64* %fdp6, align 8
%c7 = icmp slt i64 %t3, %t5
  %fdp11 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t10 = load i64, i64* %fdp11, align 8
%t13 = add i64 0, 8
%t14 = srem i64 %t10, %t13
%t16 = add i64 0, 0
%c17 = icmp eq i64 %t14, %t16
  %t1 = and i1 %c7, %c17
  %pi18 = and i1 %t1, true
  br i1 %pi18, label %ps20, label %pp19
  pp19:
    unreachable
  ps20:
  call void @llvm.assume(i1 %pi18)
  %fdp23 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t22 = load i64, i64* %fdp23, align 8
%t25 = add i64 0, 1
%t26 = add nsw i64 %t22, %t25
  %ap_27 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t26, ptr %ap_27, align 8, !tbaa !1
  %fdp31 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t30 = load i64, i64* %fdp31, align 8
%t33 = add i64 0, 5000000
%t34 = srem i64 %t30, %t33
%t36 = add i64 0, 4999999
%c37 = icmp eq i64 %t34, %t36
  br i1 %c37, label %g38_t, label %g38_e
  g38_t:
  %fdp42 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t41 = load i64, i64* %fdp42, align 8
%t44 = add i64 0, 1
%t45 = add nsw i64 %t41, %t44
    %pso46 = load volatile ptr, ptr @stdout
    %pfi47 = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0
    %ppi48 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso46, ptr %pfi47, i64 %t45)
    %t39 = zext i32 %ppi48 to i64
    br label %g38_tx
  g38_tx:
    br label %g38_e
  g38_e:
  ret void
}

define void @ack(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp53 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t52 = load i64, i64* %fdp53, align 8
  %fdp55 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t54 = load i64, i64* %fdp55, align 8
%c56 = icmp slt i64 %t52, %t54
  %fdp60 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t59 = load i64, i64* %fdp60, align 8
%t62 = add i64 0, 8
%t63 = srem i64 %t59, %t62
%t65 = add i64 0, 1
%c66 = icmp eq i64 %t63, %t65
  %t50 = and i1 %c56, %c66
  %pi67 = and i1 %t50, true
  br i1 %pi67, label %ps69, label %pp68
  pp68:
    unreachable
  ps69:
  call void @llvm.assume(i1 %pi67)
  %fdp72 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t71 = load i64, i64* %fdp72, align 8
%t74 = add i64 0, 1
%t75 = add nsw i64 %t71, %t74
  %ap_76 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t75, ptr %ap_76, align 8, !tbaa !1
  %fdp80 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t79 = load i64, i64* %fdp80, align 8
%t82 = add i64 0, 5000000
%t83 = srem i64 %t79, %t82
%t85 = add i64 0, 4999999
%c86 = icmp eq i64 %t83, %t85
  br i1 %c86, label %g87_t, label %g87_e
  g87_t:
  %fdp91 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t90 = load i64, i64* %fdp91, align 8
%t93 = add i64 0, 1
%t94 = add nsw i64 %t90, %t93
    %pso95 = load volatile ptr, ptr @stdout
    %pfi96 = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0
    %ppi97 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso95, ptr %pfi96, i64 %t94)
    %t88 = zext i32 %ppi97 to i64
    br label %g87_tx
  g87_tx:
    br label %g87_e
  g87_e:
  ret void
}

define void @err(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp102 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t101 = load i64, i64* %fdp102, align 8
  %fdp104 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t103 = load i64, i64* %fdp104, align 8
%c105 = icmp slt i64 %t101, %t103
  %fdp109 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t108 = load i64, i64* %fdp109, align 8
%t111 = add i64 0, 8
%t112 = srem i64 %t108, %t111
%t114 = add i64 0, 2
%c115 = icmp eq i64 %t112, %t114
  %t99 = and i1 %c105, %c115
  %pi116 = and i1 %t99, true
  br i1 %pi116, label %ps118, label %pp117
  pp117:
    unreachable
  ps118:
  call void @llvm.assume(i1 %pi116)
  %fdp121 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t120 = load i64, i64* %fdp121, align 8
%t123 = add i64 0, 1
%t124 = add nsw i64 %t120, %t123
  %ap_125 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t124, ptr %ap_125, align 8, !tbaa !1
  %fdp129 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t128 = load i64, i64* %fdp129, align 8
%t131 = add i64 0, 5000000
%t132 = srem i64 %t128, %t131
%t134 = add i64 0, 4999999
%c135 = icmp eq i64 %t132, %t134
  br i1 %c135, label %g136_t, label %g136_e
  g136_t:
  %fdp140 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t139 = load i64, i64* %fdp140, align 8
%t142 = add i64 0, 1
%t143 = add nsw i64 %t139, %t142
    %pso144 = load volatile ptr, ptr @stdout
    %pfi145 = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0
    %ppi146 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso144, ptr %pfi145, i64 %t143)
    %t137 = zext i32 %ppi146 to i64
    br label %g136_tx
  g136_tx:
    br label %g136_e
  g136_e:
  ret void
}

define void @debug(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp151 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t150 = load i64, i64* %fdp151, align 8
  %fdp153 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t152 = load i64, i64* %fdp153, align 8
%c154 = icmp slt i64 %t150, %t152
  %fdp158 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t157 = load i64, i64* %fdp158, align 8
%t160 = add i64 0, 8
%t161 = srem i64 %t157, %t160
%t163 = add i64 0, 3
%c164 = icmp eq i64 %t161, %t163
  %t148 = and i1 %c154, %c164
  %pi165 = and i1 %t148, true
  br i1 %pi165, label %ps167, label %pp166
  pp166:
    unreachable
  ps167:
  call void @llvm.assume(i1 %pi165)
  %fdp170 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t169 = load i64, i64* %fdp170, align 8
%t172 = add i64 0, 1
%t173 = add nsw i64 %t169, %t172
  %ap_174 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t173, ptr %ap_174, align 8, !tbaa !1
  %fdp178 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t177 = load i64, i64* %fdp178, align 8
%t180 = add i64 0, 5000000
%t181 = srem i64 %t177, %t180
%t183 = add i64 0, 4999999
%c184 = icmp eq i64 %t181, %t183
  br i1 %c184, label %g185_t, label %g185_e
  g185_t:
  %fdp189 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t188 = load i64, i64* %fdp189, align 8
%t191 = add i64 0, 1
%t192 = add nsw i64 %t188, %t191
    %pso193 = load volatile ptr, ptr @stdout
    %pfi194 = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0
    %ppi195 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso193, ptr %pfi194, i64 %t192)
    %t186 = zext i32 %ppi195 to i64
    br label %g185_tx
  g185_tx:
    br label %g185_e
  g185_e:
  ret void
}

define void @data(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp200 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t199 = load i64, i64* %fdp200, align 8
  %fdp202 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t201 = load i64, i64* %fdp202, align 8
%c203 = icmp slt i64 %t199, %t201
  %fdp207 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t206 = load i64, i64* %fdp207, align 8
%t209 = add i64 0, 8
%t210 = srem i64 %t206, %t209
%t212 = add i64 0, 4
%c213 = icmp eq i64 %t210, %t212
  %t197 = and i1 %c203, %c213
  %pi214 = and i1 %t197, true
  br i1 %pi214, label %ps216, label %pp215
  pp215:
    unreachable
  ps216:
  call void @llvm.assume(i1 %pi214)
  %fdp219 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t218 = load i64, i64* %fdp219, align 8
%t221 = add i64 0, 1
%t222 = add nsw i64 %t218, %t221
  %ap_223 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t222, ptr %ap_223, align 8, !tbaa !1
  %fdp227 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t226 = load i64, i64* %fdp227, align 8
%t229 = add i64 0, 5000000
%t230 = srem i64 %t226, %t229
%t232 = add i64 0, 4999999
%c233 = icmp eq i64 %t230, %t232
  br i1 %c233, label %g234_t, label %g234_e
  g234_t:
  %fdp238 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t237 = load i64, i64* %fdp238, align 8
%t240 = add i64 0, 1
%t241 = add nsw i64 %t237, %t240
    %pso242 = load volatile ptr, ptr @stdout
    %pfi243 = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0
    %ppi244 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso242, ptr %pfi243, i64 %t241)
    %t235 = zext i32 %ppi244 to i64
    br label %g234_tx
  g234_tx:
    br label %g234_e
  g234_e:
  ret void
}

define void @ctrl(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp249 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t248 = load i64, i64* %fdp249, align 8
  %fdp251 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t250 = load i64, i64* %fdp251, align 8
%c252 = icmp slt i64 %t248, %t250
  %fdp256 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t255 = load i64, i64* %fdp256, align 8
%t258 = add i64 0, 8
%t259 = srem i64 %t255, %t258
%t261 = add i64 0, 5
%c262 = icmp eq i64 %t259, %t261
  %t246 = and i1 %c252, %c262
  %pi263 = and i1 %t246, true
  br i1 %pi263, label %ps265, label %pp264
  pp264:
    unreachable
  ps265:
  call void @llvm.assume(i1 %pi263)
  %fdp268 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t267 = load i64, i64* %fdp268, align 8
%t270 = add i64 0, 1
%t271 = add nsw i64 %t267, %t270
  %ap_272 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t271, ptr %ap_272, align 8, !tbaa !1
  %fdp276 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t275 = load i64, i64* %fdp276, align 8
%t278 = add i64 0, 5000000
%t279 = srem i64 %t275, %t278
%t281 = add i64 0, 4999999
%c282 = icmp eq i64 %t279, %t281
  br i1 %c282, label %g283_t, label %g283_e
  g283_t:
  %fdp287 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t286 = load i64, i64* %fdp287, align 8
%t289 = add i64 0, 1
%t290 = add nsw i64 %t286, %t289
    %pso291 = load volatile ptr, ptr @stdout
    %pfi292 = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0
    %ppi293 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso291, ptr %pfi292, i64 %t290)
    %t284 = zext i32 %ppi293 to i64
    br label %g283_tx
  g283_tx:
    br label %g283_e
  g283_e:
  ret void
}

define void @sync_(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp298 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t297 = load i64, i64* %fdp298, align 8
  %fdp300 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t299 = load i64, i64* %fdp300, align 8
%c301 = icmp slt i64 %t297, %t299
  %fdp305 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t304 = load i64, i64* %fdp305, align 8
%t307 = add i64 0, 8
%t308 = srem i64 %t304, %t307
%t310 = add i64 0, 6
%c311 = icmp eq i64 %t308, %t310
  %t295 = and i1 %c301, %c311
  %pi312 = and i1 %t295, true
  br i1 %pi312, label %ps314, label %pp313
  pp313:
    unreachable
  ps314:
  call void @llvm.assume(i1 %pi312)
  %fdp317 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t316 = load i64, i64* %fdp317, align 8
%t319 = add i64 0, 1
%t320 = add nsw i64 %t316, %t319
  %ap_321 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t320, ptr %ap_321, align 8, !tbaa !1
  %fdp325 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t324 = load i64, i64* %fdp325, align 8
%t327 = add i64 0, 5000000
%t328 = srem i64 %t324, %t327
%t330 = add i64 0, 4999999
%c331 = icmp eq i64 %t328, %t330
  br i1 %c331, label %g332_t, label %g332_e
  g332_t:
  %fdp336 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t335 = load i64, i64* %fdp336, align 8
%t338 = add i64 0, 1
%t339 = add nsw i64 %t335, %t338
    %pso340 = load volatile ptr, ptr @stdout
    %pfi341 = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0
    %ppi342 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso340, ptr %pfi341, i64 %t339)
    %t333 = zext i32 %ppi342 to i64
    br label %g332_tx
  g332_tx:
    br label %g332_e
  g332_e:
  ret void
}

define void @stat(ptr noalias nocapture align 8 %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp347 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t346 = load i64, i64* %fdp347, align 8
  %fdp349 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t348 = load i64, i64* %fdp349, align 8
%c350 = icmp slt i64 %t346, %t348
  %fdp354 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t353 = load i64, i64* %fdp354, align 8
%t356 = add i64 0, 8
%t357 = srem i64 %t353, %t356
%t359 = add i64 0, 7
%c360 = icmp eq i64 %t357, %t359
  %t344 = and i1 %c350, %c360
  %pi361 = and i1 %t344, true
  br i1 %pi361, label %ps363, label %pp362
  pp362:
    unreachable
  ps363:
  call void @llvm.assume(i1 %pi361)
  %fdp366 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t365 = load i64, i64* %fdp366, align 8
%t368 = add i64 0, 1
%t369 = add nsw i64 %t365, %t368
  %ap_370 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %t369, ptr %ap_370, align 8, !tbaa !1
  %fdp374 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t373 = load i64, i64* %fdp374, align 8
%t376 = add i64 0, 5000000
%t377 = srem i64 %t373, %t376
%t379 = add i64 0, 4999999
%c380 = icmp eq i64 %t377, %t379
  br i1 %c380, label %g381_t, label %g381_e
  g381_t:
  %fdp385 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t384 = load i64, i64* %fdp385, align 8
%t387 = add i64 0, 1
%t388 = add nsw i64 %t384, %t387
    %pso389 = load volatile ptr, ptr @stdout
    %pfi390 = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0
    %ppi391 = call i32 (ptr, ptr, ...) @fprintf(ptr %pso389, ptr %pfi390, i64 %t388)
    %t382 = zext i32 %ppi391 to i64
    br label %g381_tx
  g381_tx:
    br label %g381_e
  g381_e:
  ret void
}

define internal i1 @pre_ping(ptr noalias nocapture align 8 %state) #10 {
  entry:
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 8
  %fdp6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t5 = load i64, i64* %fdp6, align 8
%c7 = icmp slt i64 %t3, %t5
  %fdp11 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t10 = load i64, i64* %fdp11, align 8
%t13 = add i64 0, 8
%t14 = srem i64 %t10, %t13
%t16 = add i64 0, 0
%c17 = icmp eq i64 %t14, %t16
  %t1 = and i1 %c7, %c17
  ret i1 %t1
}
define internal i1 @pre_ack(ptr noalias nocapture align 8 %state) #10 {
  entry:
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 8
  %fdp6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t5 = load i64, i64* %fdp6, align 8
%c7 = icmp slt i64 %t3, %t5
  %fdp11 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t10 = load i64, i64* %fdp11, align 8
%t13 = add i64 0, 8
%t14 = srem i64 %t10, %t13
%t16 = add i64 0, 1
%c17 = icmp eq i64 %t14, %t16
  %t1 = and i1 %c7, %c17
  ret i1 %t1
}
define internal i1 @pre_err(ptr noalias nocapture align 8 %state) #10 {
  entry:
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 8
  %fdp6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t5 = load i64, i64* %fdp6, align 8
%c7 = icmp slt i64 %t3, %t5
  %fdp11 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t10 = load i64, i64* %fdp11, align 8
%t13 = add i64 0, 8
%t14 = srem i64 %t10, %t13
%t16 = add i64 0, 2
%c17 = icmp eq i64 %t14, %t16
  %t1 = and i1 %c7, %c17
  ret i1 %t1
}
define internal i1 @pre_debug(ptr noalias nocapture align 8 %state) #10 {
  entry:
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 8
  %fdp6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t5 = load i64, i64* %fdp6, align 8
%c7 = icmp slt i64 %t3, %t5
  %fdp11 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t10 = load i64, i64* %fdp11, align 8
%t13 = add i64 0, 8
%t14 = srem i64 %t10, %t13
%t16 = add i64 0, 3
%c17 = icmp eq i64 %t14, %t16
  %t1 = and i1 %c7, %c17
  ret i1 %t1
}
define internal i1 @pre_data(ptr noalias nocapture align 8 %state) #10 {
  entry:
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 8
  %fdp6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t5 = load i64, i64* %fdp6, align 8
%c7 = icmp slt i64 %t3, %t5
  %fdp11 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t10 = load i64, i64* %fdp11, align 8
%t13 = add i64 0, 8
%t14 = srem i64 %t10, %t13
%t16 = add i64 0, 4
%c17 = icmp eq i64 %t14, %t16
  %t1 = and i1 %c7, %c17
  ret i1 %t1
}
define internal i1 @pre_ctrl(ptr noalias nocapture align 8 %state) #10 {
  entry:
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 8
  %fdp6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t5 = load i64, i64* %fdp6, align 8
%c7 = icmp slt i64 %t3, %t5
  %fdp11 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t10 = load i64, i64* %fdp11, align 8
%t13 = add i64 0, 8
%t14 = srem i64 %t10, %t13
%t16 = add i64 0, 5
%c17 = icmp eq i64 %t14, %t16
  %t1 = and i1 %c7, %c17
  ret i1 %t1
}
define internal i1 @pre_sync_(ptr noalias nocapture align 8 %state) #10 {
  entry:
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 8
  %fdp6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t5 = load i64, i64* %fdp6, align 8
%c7 = icmp slt i64 %t3, %t5
  %fdp11 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t10 = load i64, i64* %fdp11, align 8
%t13 = add i64 0, 8
%t14 = srem i64 %t10, %t13
%t16 = add i64 0, 6
%c17 = icmp eq i64 %t14, %t16
  %t1 = and i1 %c7, %c17
  ret i1 %t1
}
define internal i1 @pre_stat(ptr noalias nocapture align 8 %state) #10 {
  entry:
  %fdp4 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 8
  %fdp6 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %t5 = load i64, i64* %fdp6, align 8
%c7 = icmp slt i64 %t3, %t5
  %fdp11 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  %t10 = load i64, i64* %fdp11, align 8
%t13 = add i64 0, 8
%t14 = srem i64 %t10, %t13
%t16 = add i64 0, 7
%c17 = icmp eq i64 %t14, %t16
  %t1 = and i1 %c7, %c17
  ret i1 %t1
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
  store i64 0, ptr %ip_2, align 8
  ret void
}


define i32 @main() local_unnamed_addr #9 {
  entry:
  %state_0 = alloca %StateChunk0, align 8
  %state = alloca %State, align 8
  %ip_16 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
%t18 = add i64 0, 0
  store i64 %t18, ptr %ip_16, align 8
  %ip_19 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
%sp23 = getelementptr inbounds [6 x i8], [6 x i8]* @str.3, i64 0, i64 0
%t22 = ptrtoint i8* %sp23 to i64
  %gsr24 = inttoptr i64 %t22 to ptr
  %gsp25 = bitcast ptr %gsr24 to ptr
  %gdp26 = load i64, ptr %gsp25, align 8
  %gnp27 = inttoptr i64 %gdp26 to ptr
  %gnv28 = call ptr @getenv(ptr %gnp27)
  %gnvl29 = icmp eq ptr %gnv28, null
  br i1 %gnvl29, label %genv_nul30, label %genv_ok31
  genv_nul30:
    br label %genv_af32
  genv_ok31:
  %gav33 = call i64 @atol(ptr %gnv28)
    br label %genv_af32
  genv_af32:
  %t20 = phi i64 [ 0, %genv_nul30 ], [ %gav33, %genv_ok31 ]
  store i64 %t20, ptr %ip_19, align 8
  %ip_34 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  store i64 0, ptr %ip_34, align 8
  %c2m_s35 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 0
  %c2m_v36 = load i64, ptr %c2m_s35, align 8
  %c2m_d37 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  store i64 %c2m_v36, ptr %c2m_d37, align 8
  %c2m_s38 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 1
  %c2m_v39 = load i64, ptr %c2m_s38, align 8
  %c2m_d40 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  store i64 %c2m_v39, ptr %c2m_d40, align 8
  %c2m_s41 = getelementptr inbounds %StateChunk0, ptr %state_0, i32 0, i32 2
  %c2m_v42 = load i64, ptr %c2m_s41, align 8
  %c2m_d43 = getelementptr inbounds %State, ptr %state, i32 0, i32 2
  store i64 %c2m_v42, ptr %c2m_d43, align 8
  %arptr3 = alloca i8*, align 8
  %arend3 = alloca i8*, align 8
  %arbase3 = alloca i8*, align 8
  %arinit3 = call ptr @malloc(i64 65536)
  store i8* %arinit3, ptr %arptr3, align 8
  store i8* %arinit3, ptr %arbase3, align 8
  %arieu3 = getelementptr i8, ptr %arinit3, i64 65536
  store i8* %arieu3, ptr %arend3, align 8
  %gep_bn44 = getelementptr inbounds %State, ptr %state, i32 0, i32 1
  %val_bn45 = load i64, ptr %gep_bn44, align 8
  %cgep_base46 = getelementptr inbounds %State, ptr %state, i32 0, i32 0
  br label %_body4
_body4:
  %cc47 = load i64, ptr %cgep_base46, align 8
  %ci48 = add i64 %cc47, 8
  store i64 %ci48, ptr %cgep_base46, align 8
  %cm49 = srem i64 %ci48, 5000000
  %cg50 = icmp eq i64 %cm49, 0
  br i1 %cg50, label %pb, label %pe
pb:
  %so51 = load volatile ptr, ptr @stdout
  %fg52 = getelementptr [5 x i8], [5 x i8]* @FMT_INT, i64 0, i64 0
  %pi53 = call i32 (ptr, ptr, ...) @fprintf(ptr %so51, ptr %fg52, i64 %ci48)
  br label %pe
pe:
  %cnt_check54 = load i64, ptr %cgep_base46, align 8
  %cont55 = icmp slt i64 %cnt_check54, %val_bn45
  br i1 %cont55, label %_body4, label %_done
_done:
  %arr4 = load ptr, ptr %arbase3, align 8
  store i8* %arr4, ptr %arptr3, align 8
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

!0 = !{!"Briev"}
!1 = !{!"Int", !0}
!2 = !{!"Char", !0}
!3 = !{!"Float", !0}
!4 = !{!"Bool", !0}
!5 = !{!"String", !0}
!99 = distinct !{} ; StateAliasScope
