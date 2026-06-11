; ModuleID = 'program.ll'
source_filename = "program.bv"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

declare void @llvm.assume(i1) #1
declare float @llvm.sqrt.f32(float) #1
declare float @llvm.fabs.f32(float) #1
declare float @llvm.ceil.f32(float) #1
declare float @llvm.floor.f32(float) #1
declare i64 @llvm.ctpop.i64(i64) #1
declare i64 @llvm.ctlz.i64(i64, i1) #1
declare i64 @llvm.cttz.i64(i64, i1) #1
declare i64 @llvm.abs.i64(i64, i1) #1
declare i64 @llvm.bitreverse.i64(i64) #1
declare void @brief_barrier_release()
declare void @brief_barrier_wait()
declare void @brief_thread_pool_init(i32, i8**)
declare i64 @__get_env_int(i8*) #1
declare i64 @__print_int(i64) #1
%State = type { i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant [6 x i8] c"BOUND\00", align 1

define void @work(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t1 = load i64, i64* %fdp2, align 8
  %fdp4 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t3 = load i64, i64* %fdp4, align 8
  %c5 = icmp slt i64 %t1, %t3
  %t6 = zext i1 %c5 to i64
  %pi7 = icmp ne i64 %t6, 0
  br i1 %pi7, label %ps9, label %pp8
  pp8:
    unreachable
  ps9:
  call void @llvm.assume(i1 %pi7)
  %fdp12 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t11 = load i64, i64* %fdp12, align 8
%t14 = add i64 0, 1
  %t15 = add i64 %t11, %t14
  %ap16 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store i64 %t15, i64* %ap16, align 8
  %fdp20 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t19 = load i64, i64* %fdp20, align 8
%t22 = add i64 0, 5000000
  %t18 = srem i64 %t19, %t22
%t24 = add i64 0, 0
  %c25 = icmp eq i64 %t18, %t24
  %t26 = zext i1 %c25 to i64
  %gc27 = icmp ne i64 %t26, 0
  br i1 %gc27, label %g28_t, label %g28_e
  g28_t:
    %fdp31 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
    %t30 = load i64, i64* %fdp31, align 8
    %t29 = call i64 @__print_int(i64 %t30)
    br label %g28_e
  g28_e:
  ret void
}

define internal i1 @pre_work(%State* noalias nocapture %state) #0 {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t1 = load i64, i64* %fdp2, align 8
  %fdp4 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t3 = load i64, i64* %fdp4, align 8
  %c5 = icmp slt i64 %t1, %t3
  %t6 = zext i1 %c5 to i64
  %ri7 = icmp ne i64 %t6, 0
  ret i1 %ri7
}
define void @init_state(%State* noalias nocapture %state) local_unnamed_addr #0 {
  entry:
  %ip0 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
%t1 = add i64 0, 0
  store i64 %t1, i64* %ip0, align 8
  %ip1 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
%sp5 = getelementptr inbounds [6 x i8], [6 x i8]* @str.0, i64 0, i64 0
%t4 = ptrtoint i8* %sp5 to i64
  %fp6 = inttoptr i64 %t4 to i8*
  %t2 = call i64 @__get_env_int(i8* %fp6)
  store i64 %t2, i64* %ip1, align 8
  ret void
}

define i32 @main() local_unnamed_addr #3 {
  entry:
  %state = alloca %State, align 8
  call void @init_state(%State* noalias nocapture %state)
  br label %tick
  tick:
  %gep_N_7 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %N_old_8 = load i64, i64* %gep_N_7, align 8
  %gep_ops_9 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %ops_old_10 = load i64, i64* %gep_ops_9, align 8
  %t12 = add i64 0, %ops_old_10
  %t13 = add i64 0, %N_old_8
  %c14 = icmp slt i64 %t12, %t13
  %t15 = zext i1 %c14 to i64
  %pi16 = icmp ne i64 %t15, 0
  br i1 %pi16, label %b_work, label %s_work
  b_work:
  %gep_N_17 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %N_old_18 = load i64, i64* %gep_N_17, align 8
  %gep_ops_19 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %ops_old_20 = load i64, i64* %gep_ops_19, align 8
  %t22 = add i64 0, %ops_old_20
%t24 = add i64 0, 1
  %t25 = add i64 %t22, %t24
  %ap26 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store i64 %t25, i64* %ap26, align 8
  %t29 = add i64 0, %ops_old_20
%t31 = add i64 0, 5000000
  %t28 = srem i64 %t29, %t31
%t33 = add i64 0, 0
  %c34 = icmp eq i64 %t28, %t33
  %t35 = zext i1 %c34 to i64
  %gc36 = icmp ne i64 %t35, 0
  br i1 %gc36, label %g37_t, label %g37_e
  g37_t:
    %t39 = add i64 0, %ops_old_20
    %t38 = call i64 @__print_int(i64 %t39)
    br label %g37_e
  g37_e:
  br label %s_work
  s_work:
  %gep_exit_42 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t41 = load i64, i64* %gep_exit_42, align 8
  %gep_exit_44 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t43 = load i64, i64* %gep_exit_44, align 8
  %t45 = icmp eq i64 %t41, %t43
  %t40 = zext i1 %t45 to i64
  %t46 = trunc i64 %t40 to i1
  br i1 %t46, label %done, label %tick
  done:
  ret i32 0
}


attributes #0 = {
    mustprogress nofree norecurse nosync nounwind willreturn
    memory(argmem: readwrite)
}
attributes #1 = { nocallback nofree nosync nounwind willreturn }
attributes #2 = { mustprogress nofree norecurse nosync nounwind memory(readwrite) }
attributes #3 = { nofree norecurse nosync nounwind memory(readwrite) }
