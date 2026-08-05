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
declare void @briv_barrier_release()
declare void @briv_barrier_wait()
declare void @briv_thread_pool_init(i32, i8**)
declare i64 @__print_int(i64) #1
%State = type { i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

define void @__init(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp3 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t2 = load i64, i64* %fdp3, align 8
  %t0 = xor i64 %t2, 1
  %pi4 = icmp ne i64 %t0, 0
  br i1 %pi4, label %ps6, label %pp5
  pp5:
    unreachable
  ps6:
  call void @llvm.assume(i1 %pi4)
  %fdp9 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t8 = load i64, i64* %fdp9, align 8
  %t7 = call i64 @__print_int(i64 %t8)
%t11 = add i64 0, 42
  %ap12 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  store i64 %t11, i64* %ap12, align 8
  %fdp15 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t14 = load i64, i64* %fdp15, align 8
  %t13 = call i64 @__print_int(i64 %t14)
  %t16 = add i64 0, 1
  %ap17 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store i64 %t16, i64* %ap17, align 8
  ret void
}

define internal i1 @pre___init(%State* noalias nocapture %state) #0 {
  entry:
  %fdp3 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t2 = load i64, i64* %fdp3, align 8
  %t0 = xor i64 %t2, 1
  %ri4 = icmp ne i64 %t0, 0
  ret i1 %ri4
}
define void @init_state(%State* noalias nocapture %state) local_unnamed_addr #0 {
  entry:
  %ip0 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store i64 0, i64* %ip0, align 8
  %ip1 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
%t1 = add i64 0, 0
  store i64 %t1, i64* %ip1, align 8
  ret void
}

define i32 @main() local_unnamed_addr #3 {
  entry:
  %state = alloca %State, align 8
  call void @init_state(%State* noalias nocapture %state)
  br label %tick
  tick:
  %gep_x_2 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %x_old_3 = load i64, i64* %gep_x_2, align 8
  %gep___booted_0_4 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %__booted_0_old_5 = load i64, i64* %gep___booted_0_4, align 8
  %t8 = add i64 0, %__booted_0_old_5
  %t6 = xor i64 %t8, 1
  %pi9 = icmp ne i64 %t6, 0
  br i1 %pi9, label %b___init, label %s___init
  b___init:
  %gep_x_10 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %x_old_11 = load i64, i64* %gep_x_10, align 8
  %gep___booted_0_12 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %__booted_0_old_13 = load i64, i64* %gep___booted_0_12, align 8
  %t15 = add i64 0, %x_old_11
  %t14 = call i64 @__print_int(i64 %t15)
%t17 = add i64 0, 42
  %ap18 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  store i64 %t17, i64* %ap18, align 8
  %t20 = add i64 0, %x_old_11
  %t19 = call i64 @__print_int(i64 %t20)
  %t21 = add i64 0, 1
  %ap22 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store i64 %t21, i64* %ap22, align 8
  br label %s___init
  s___init:
  br label %tick
  ret i32 0
}


attributes #0 = {
    mustprogress nofree norecurse nosync nounwind willreturn
    memory(argmem: readwrite)
}
attributes #1 = { nocallback nofree nosync nounwind willreturn }
attributes #2 = { mustprogress nofree norecurse nosync nounwind memory(readwrite) }
attributes #3 = { nofree norecurse nosync nounwind memory(readwrite) }
