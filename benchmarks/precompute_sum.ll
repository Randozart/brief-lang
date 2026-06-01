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

@total = constant i64 500

%State = type { i64, i64, i64 }
@global_state = global %State zeroinitializer

define void @step_a(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il3 = load i64, i64* %fdp2, align 8
  %t1 = add i64 0, %il3
  %il5 = load i64, i64* @total, align 8
  %t4 = add i64 0, %il5
  %c6 = icmp slt i64 %t1, %t4
  %t0 = zext i1 %c6 to i64
  %pi7 = icmp ne i64 %t0, 0
  br i1 %pi7, label %ps9, label %pp8
  pp8:
    unreachable
  ps9:
  call void @llvm.assume(i1 %pi7)
  %fdp12 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il13 = load i64, i64* %fdp12, align 8
  %t11 = add i64 0, %il13
  %fdp15 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il16 = load i64, i64* %fdp15, align 8
  %t14 = add i64 0, %il16
  %t10 = add i64 %t11, %t14
  %ap17 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  store i64 %t10, i64* %ap17, align 8
  %fdp20 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il21 = load i64, i64* %fdp20, align 8
  %t19 = add i64 0, %il21
  %t22 = add i64 0, 1
  %t18 = add i64 %t19, %t22
  %ap23 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store i64 %t18, i64* %ap23, align 8
  ret void
}

define void @step_b(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il3 = load i64, i64* %fdp2, align 8
  %t1 = add i64 0, %il3
  %il5 = load i64, i64* @total, align 8
  %t4 = add i64 0, %il5
  %c6 = icmp slt i64 %t1, %t4
  %t0 = zext i1 %c6 to i64
  %pi7 = icmp ne i64 %t0, 0
  br i1 %pi7, label %ps9, label %pp8
  pp8:
    unreachable
  ps9:
  call void @llvm.assume(i1 %pi7)
  %fdp12 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il13 = load i64, i64* %fdp12, align 8
  %t11 = add i64 0, %il13
  %fdp15 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il16 = load i64, i64* %fdp15, align 8
  %t14 = add i64 0, %il16
  %t10 = add i64 %t11, %t14
  %ap17 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  store i64 %t10, i64* %ap17, align 8
  %fdp20 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il21 = load i64, i64* %fdp20, align 8
  %t19 = add i64 0, %il21
  %t22 = add i64 0, 1
  %t18 = add i64 %t19, %t22
  %ap23 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store i64 %t18, i64* %ap23, align 8
  ret void
}

define internal i1 @pre_step_a(%State* noalias nocapture %state) #0 {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il3 = load i64, i64* %fdp2, align 8
  %t1 = add i64 0, %il3
  %il5 = load i64, i64* @total, align 8
  %t4 = add i64 0, %il5
  %c6 = icmp slt i64 %t1, %t4
  %t0 = zext i1 %c6 to i64
  %ri7 = icmp ne i64 %t0, 0
  ret i1 %ri7
}
define internal i1 @pre_step_b(%State* noalias nocapture %state) #0 {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il3 = load i64, i64* %fdp2, align 8
  %t1 = add i64 0, %il3
  %il5 = load i64, i64* @total, align 8
  %t4 = add i64 0, %il5
  %c6 = icmp slt i64 %t1, %t4
  %t0 = zext i1 %c6 to i64
  %ri7 = icmp ne i64 %t0, 0
  ret i1 %ri7
}
define void @init_state() local_unnamed_addr #0 {
  entry:
  %ip0 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 0
  store volatile i64 0, i64* %ip0, align 8
  %ip1 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 1
  store volatile i64 0, i64* %ip1, align 8
  %ip2 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 2
  store volatile i64 0, i64* %ip2, align 8
  ret void
}

define i32 @main() local_unnamed_addr #0 {
  entry:
  call void @init_state()
  %gp_count = getelementptr inbounds %State, %State* @global_state, i32 0, i32 0
  store i64 500, i64* %gp_count, align 8
  ret i32 0
}


attributes #0 = {
    mustprogress nofree norecurse nosync nounwind willreturn
    memory(argmem: readwrite)
}
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: write) }
attributes #2 = { mustprogress nofree norecurse nosync nounwind memory(readwrite) }
attributes #3 = { nofree norecurse nosync nounwind memory(readwrite) }
