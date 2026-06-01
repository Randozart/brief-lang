; ModuleID = 'program.ll'
source_filename = "program.bv"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

declare void @llvm.assume(i1) #1
declare void @__rt_init() local_unnamed_addr
declare void @__rt_wait() local_unnamed_addr
declare void @brief_thread_pool_init(i32, i8**) local_unnamed_addr
declare void @brief_barrier_release() local_unnamed_addr
declare void @brief_barrier_wait() local_unnamed_addr
@__io_pending = external global i8, align 1


@N = constant i64 50000000

%State = type { i64 }
@global_state = global %State zeroinitializer

define void @work(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %tr2 = load volatile i8, i8* @__io_pending
  %tz3 = zext i8 %tr2 to i64
  %t1 = add i64 0, %tz3
  %fdp6 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il7 = load i64, i64* %fdp6, align 8
  %t5 = add i64 0, %il7
  %il9 = load i64, i64* @N, align 8
  %t8 = add i64 0, %il9
  %c10 = icmp slt i64 %t5, %t8
  %t4 = zext i1 %c10 to i64
  %t0 = and i64 %t1, %t4
  %pi11 = icmp ne i64 %t0, 0
  br i1 %pi11, label %ps13, label %pp12
  pp12:
    unreachable
  ps13:
  call void @llvm.assume(i1 %pi11)
  %fdp16 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il17 = load i64, i64* %fdp16, align 8
  %t15 = add i64 0, %il17
  %t18 = add i64 0, 1
  %t14 = add i64 %t15, %t18
  %ap19 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store i64 %t14, i64* %ap19, align 8
  ret void
}

define internal i1 @pre_work(%State* noalias nocapture %state) #0 {
  entry:
  %tr2 = load volatile i8, i8* @__io_pending
  %tz3 = zext i8 %tr2 to i64
  %t1 = add i64 0, %tz3
  %fdp6 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il7 = load i64, i64* %fdp6, align 8
  %t5 = add i64 0, %il7
  %il9 = load i64, i64* @N, align 8
  %t8 = add i64 0, %il9
  %c10 = icmp slt i64 %t5, %t8
  %t4 = zext i1 %c10 to i64
  %t0 = and i64 %t1, %t4
  %ri11 = icmp ne i64 %t0, 0
  ret i1 %ri11
}
define void @init_state() local_unnamed_addr #0 {
  entry:
  %ip0 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 0
  store volatile i64 0, i64* %ip0, align 8
  ret void
}

define void @reactor_tick() local_unnamed_addr #2 {
  entry:
  %tr12 = load volatile i8, i8* @__io_pending
  %tz13 = zext i8 %tr12 to i64
  %sz_io_pending = add i64 0, %tz13
  %fired_mask = alloca i64, align 8
  store i64 0, i64* %fired_mask
  %pr0 = call i1 @pre_work(%State* @global_state)
  br i1 %pr0, label %b0, label %ck1
b0:
  call void @work(%State* @global_state)
  %fm0a = load i64, i64* %fired_mask
  %fm0b = or i64 %fm0a, 1
  store i64 %fm0b, i64* %fired_mask
  br label %ck1
ck1:
  ret void
}

define i32 @main() local_unnamed_addr #3 {
  entry:
  call void @init_state()
  call void @__rt_init()
  br label %tick
tick:
  %tr14 = load volatile i8, i8* @__io_pending
  %tz15 = zext i8 %tr14 to i64
  %sz_io_pending = add i64 0, %tz15
  switch i64 %sz_io_pending, label %io_pending_residual [
    i64 0, label %io_pending_case_0
    i64 1, label %io_pending_case_1
  ]
io_pending_case_0:
  %ltio_pending_0_16 = load i64, i64* @N, align 8
  br label %io_pending_0_hdr
io_pending_0_hdr:
  %gpio_pending_0_17 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 0
  %lpio_pending_0_17 = load i64, i64* %gpio_pending_0_17, align 8
  %cpio_pending_0_18 = icmp slt i64 %lpio_pending_0_17, %ltio_pending_0_16
  br i1 %cpio_pending_0_18, label %io_pending_0_body, label %io_pending_0_done
io_pending_0_body:
  call void @work(%State* @global_state)
  br label %io_pending_0_hdr
io_pending_0_done:
  br label %exit_check
io_pending_case_1:
  %ltio_pending_1_16 = load i64, i64* @N, align 8
  br label %io_pending_1_hdr
io_pending_1_hdr:
  %gpio_pending_1_17 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 0
  %lpio_pending_1_17 = load i64, i64* %gpio_pending_1_17, align 8
  %cpio_pending_1_18 = icmp slt i64 %lpio_pending_1_17, %ltio_pending_1_16
  br i1 %cpio_pending_1_18, label %io_pending_1_body, label %io_pending_1_done
io_pending_1_body:
  call void @work(%State* @global_state)
  br label %io_pending_1_hdr
io_pending_1_done:
  br label %exit_check
io_pending_residual:
  call void @reactor_tick()
  br label %exit_check
exit_check:
  %gep_exit_18 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 0
  %t17 = load i64, i64* %gep_exit_18, align 8
  %t19 = load i64, i64* @N, align 8
  %t20 = icmp eq i64 %t17, %t19
  %t16 = zext i1 %t20 to i64
  %t21 = trunc i64 %t16 to i1
  br i1 %t21, label %done, label %do_wait
do_wait:
  call void @__rt_wait()
  br label %tick
done:
  ret i32 0
}

@llvm.wake_triggers = constant [1 x i8*] [i8* @__io_pending]
!llvm.wake_triggers = !{!0}
!0 = !{!"__io_pending"}

attributes #0 = {
    mustprogress nofree norecurse nosync nounwind willreturn
    memory(argmem: readwrite)
}
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: write) }
attributes #2 = { mustprogress nofree norecurse nosync nounwind memory(readwrite) }
attributes #3 = { nofree norecurse nosync nounwind memory(readwrite) }
