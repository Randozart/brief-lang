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
declare void @briev_barrier_release()
declare void @briev_barrier_wait()
declare void @briev_thread_pool_init(i32, i8**)
declare i64 @__print_int(i64) #1
declare i64 @__print_float(float) #1
%State = type { i64, i64, i64, i64, i8, i8, i8 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant [5 x i8] c"[15]\00", align 1

define void @init_state(%State* noalias nocapture %state) local_unnamed_addr #0 {
  entry:
  %ip0 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
%t1 = add i64 0, 15561
  store i64 %t1, i64* %ip0, align 8
  %ip1 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %fdp4 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t3 = load i64, i64* %fdp4, align 8
  %mhp5 = inttoptr i64 %t3 to i64*
  %mdp6 = load i64, i64* %mhp5, align 8
  %mde7 = inttoptr i64 %mdp6 to i64*
  %t2 = add i64 0, 0 ; multislice
  store i64 %t2, i64* %ip1, align 8
  %ip2 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %fdp10 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t9 = load i64, i64* %fdp10, align 8
  %mhp11 = inttoptr i64 %t9 to i64*
  %mdp12 = load i64, i64* %mhp11, align 8
  %mde13 = inttoptr i64 %mdp12 to i64*
  %t8 = add i64 0, 0 ; multislice
  store i64 %t8, i64* %ip2, align 8
  %ip3 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  %fdp16 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t15 = load i64, i64* %fdp16, align 8
  %mhp17 = inttoptr i64 %t15 to i64*
  %mdp18 = load i64, i64* %mhp17, align 8
  %mde19 = inttoptr i64 %mdp18 to i64*
  %t14 = add i64 0, 0 ; multislice
  store i64 %t14, i64* %ip3, align 8
  %ip4 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  %fdp22 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t21 = load i64, i64* %fdp22, align 8
  %t20 = call i64 @__print_int(i64 %t21)
  %ip5t = trunc i64 %t20 to i8
  store i8 %ip5t, i8* %ip4, align 1
  %ip6 = getelementptr inbounds %State, %State* %state, i32 0, i32 5
  %fdp25 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %t24 = load i64, i64* %fdp25, align 8
  %t23 = call i64 @__print_int(i64 %t24)
  %ip7t = trunc i64 %t23 to i8
  store i8 %ip7t, i8* %ip6, align 1
  %ip8 = getelementptr inbounds %State, %State* %state, i32 0, i32 6
  %fdp28 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  %t27 = load i64, i64* %fdp28, align 8
  %t26 = call i64 @__print_int(i64 %t27)
  %ip9t = trunc i64 %t26 to i8
  store i8 %ip9t, i8* %ip8, align 1
  ret void
}

define void @reactor_tick(%State* noalias nocapture %state) local_unnamed_addr #2 {
  entry:
  ret void
}

define i32 @main() local_unnamed_addr #3 {
  entry:
  %state = alloca %State, align 8
  call void @init_state(%State* noalias nocapture %state)
  br label %tick
  tick:
  call void @reactor_tick(%State* noalias nocapture %state)
  %t29 = add i64 0, 0 ; unsupported exit expr
  %t30 = trunc i64 %t29 to i1
  br i1 %t30, label %done, label %tick
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
