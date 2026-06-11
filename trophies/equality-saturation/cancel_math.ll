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
declare i64 @__print_int(i64) #1
declare i64 @__get_env_int(i8*) #1
@R = constant i64 100

%State = type { i64, i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant [6 x i8] c"BOUND\00", align 1

define void @step(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t1 = load i64, i64* %fdp2, align 8
  %fdp4 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 8
  %c5 = icmp slt i64 %t1, %t3
  %t6 = zext i1 %c5 to i64
  %pi7 = icmp ne i64 %t6, 0
  br i1 %pi7, label %ps9, label %pp8
  pp8:
    unreachable
  ps9:
  call void @llvm.assume(i1 %pi7)
  %fdp12 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %t11 = load i64, i64* %fdp12, align 8
  %fdp14 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t13 = load i64, i64* %fdp14, align 8
  %t15 = add i64 %t11, %t13
  %ap16 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  store i64 %t15, i64* %ap16, align 8
  %fdp19 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t18 = load i64, i64* %fdp19, align 8
%t21 = add i64 0, 1
  %t22 = add i64 %t18, %t21
  %ap23 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  store i64 %t22, i64* %ap23, align 8
  %fdp27 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t26 = load i64, i64* %fdp27, align 8
%t29 = add i64 0, 5000000
  %t25 = srem i64 %t26, %t29
%t31 = add i64 0, 0
  %c32 = icmp eq i64 %t25, %t31
  %t33 = zext i1 %c32 to i64
  %gc34 = icmp ne i64 %t33, 0
  br i1 %gc34, label %g35_t, label %g35_e
  g35_t:
    %fdp38 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
    %t37 = load i64, i64* %fdp38, align 8
    %t36 = call i64 @__print_int(i64 %t37)
    br label %g35_e
  g35_e:
  ret void
}

define internal i1 @pre_step(%State* noalias nocapture %state) #0 {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t1 = load i64, i64* %fdp2, align 8
  %fdp4 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 8
  %c5 = icmp slt i64 %t1, %t3
  %t6 = zext i1 %c5 to i64
  %ri7 = icmp ne i64 %t6, 0
  ret i1 %ri7
}
define void @init_state(%State* noalias nocapture %state) local_unnamed_addr #0 {
  entry:
  %ip0 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
%sp3 = getelementptr inbounds [6 x i8], [6 x i8]* @str.0, i64 0, i64 0
%t2 = ptrtoint i8* %sp3 to i64
  %fp4 = inttoptr i64 %t2 to i8*
  %t0 = call i64 @__get_env_int(i8* %fp4)
  store i64 %t0, i64* %ip0, align 8
  %ip1 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
%t6 = add i64 0, 0
  store i64 %t6, i64* %ip1, align 8
  %ip2 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
%t8 = add i64 0, 0
  store i64 %t8, i64* %ip2, align 8
  ret void
}

define i32 @main() local_unnamed_addr #3 {
  entry:
  %state = alloca %State, align 8
  call void @init_state(%State* noalias nocapture %state)
  br label %tick
  tick:
  %gep_acc_9 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %acc_old_10 = load i64, i64* %gep_acc_9, align 8
  %gep_N_11 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %N_old_12 = load i64, i64* %gep_N_11, align 8
  %gep_count_13 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %count_old_14 = load i64, i64* %gep_count_13, align 8
  %t16 = add i64 0, %count_old_14
  %t17 = add i64 0, %N_old_12
  %c18 = icmp slt i64 %t16, %t17
  %t19 = zext i1 %c18 to i64
  %pi20 = icmp ne i64 %t19, 0
  br i1 %pi20, label %b_step, label %s_step
  b_step:
  %gep_acc_21 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %acc_old_22 = load i64, i64* %gep_acc_21, align 8
  %gep_N_23 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %N_old_24 = load i64, i64* %gep_N_23, align 8
  %gep_count_25 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %count_old_26 = load i64, i64* %gep_count_25, align 8
  %t28 = add i64 0, %acc_old_22
  %t29 = add i64 0, %count_old_26
  %t30 = add i64 %t28, %t29
  %ap31 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  store i64 %t30, i64* %ap31, align 8
  %t33 = add i64 0, %count_old_26
%t35 = add i64 0, 1
  %t36 = add i64 %t33, %t35
  %ap37 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  store i64 %t36, i64* %ap37, align 8
  %t40 = add i64 0, %count_old_26
%t42 = add i64 0, 5000000
  %t39 = srem i64 %t40, %t42
%t44 = add i64 0, 0
  %c45 = icmp eq i64 %t39, %t44
  %t46 = zext i1 %c45 to i64
  %gc47 = icmp ne i64 %t46, 0
  br i1 %gc47, label %g48_t, label %g48_e
  g48_t:
    %t50 = add i64 0, %acc_old_22
    %t49 = call i64 @__print_int(i64 %t50)
    br label %g48_e
  g48_e:
  br label %s_step
  s_step:
  %gep_exit_54 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t53 = load i64, i64* %gep_exit_54, align 8
  %gep_exit_56 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t55 = load i64, i64* %gep_exit_56, align 8
  %t57 = icmp eq i64 %t53, %t55
  %t52 = zext i1 %t57 to i64
  %gep_exit_60 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %t59 = load i64, i64* %gep_exit_60, align 8
  %t61 = add i64 0, 0 ; unsupported exit expr
  %t62 = icmp sge i64 %t59, %t61
  %t58 = zext i1 %t62 to i64
  %t51 = and i64 %t52, %t58
  %t63 = trunc i64 %t51 to i1
  br i1 %t63, label %done, label %tick
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
