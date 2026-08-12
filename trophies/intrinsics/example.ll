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
declare i64 @__print_float(float) #1
declare i64 @__get_env_int(i8*) #1
%State = type { i8, i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant [6 x i8] c"BOUND\00", align 1

define void @compute(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t1 = load i64, i64* %fdp2, align 8
  %fdp4 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %t3 = load i64, i64* %fdp4, align 8
  %c5 = icmp slt i64 %t1, %t3
  %t6 = zext i1 %c5 to i64
  %pi7 = icmp ne i64 %t6, 0
  br i1 %pi7, label %ps9, label %pp8
  pp8:
    unreachable
  ps9:
  call void @llvm.assume(i1 %pi7)
%ff12 = bitcast i32 1077936128 to float
%fi13 = bitcast float %ff12 to i32
%t11 = zext i32 %fi13 to i64
  ; let x = %t11
%ff16 = bitcast i32 1082130432 to float
%fi17 = bitcast float %ff16 to i32
%t15 = zext i32 %fi17 to i64
  ; let y = %t15
  %bfr22 = fmul fast float %ff12, %ff12
  %bfr26 = fmul fast float %ff16, %ff16
  %bfr27 = fadd fast float %bfr22, %bfr26
  ; let dsq = %bfr27
  %t28 = call float @llvm.sqrt.f32(float %bfr27)
  ; let d = %t28
  %t30 = call i64 @__print_float(float %t28)
  ; let _p = %t30
  %fdp34 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t33 = load i64, i64* %fdp34, align 8
%t36 = add i64 0, 1
  %t37 = add i64 %t33, %t36
  %ap38 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  store i64 %t37, i64* %ap38, align 8
  ret void
}

define void @__init(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp3 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il4 = load i8, i8* %fdp3, align 1
  %t2 = zext i8 %il4 to i64
  %t0 = xor i64 %t2, 1
  %pi5 = icmp ne i64 %t0, 0
  br i1 %pi5, label %ps7, label %pp6
  pp6:
    unreachable
  ps7:
  call void @llvm.assume(i1 %pi5)
%ff12 = bitcast i32 1073741824 to float
%fi13 = bitcast float %ff12 to i32
%t11 = zext i32 %fi13 to i64
  %t9 = call float @llvm.sqrt.f32(float %ff12)
  %t8 = call i64 @__print_float(float %t9)
  ret void
  %t14 = add i64 0, 1
  %ap15 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %tr16 = trunc i64 %t14 to i8
  store i8 %tr16, i8* %ap15, align 1
  ret void
}

define internal i1 @pre_compute(%State* noalias nocapture %state) #0 {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t1 = load i64, i64* %fdp2, align 8
  %fdp4 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %t3 = load i64, i64* %fdp4, align 8
  %c5 = icmp slt i64 %t1, %t3
  %t6 = zext i1 %c5 to i64
  %ri7 = icmp ne i64 %t6, 0
  ret i1 %ri7
}
define internal i1 @pre___init(%State* noalias nocapture %state) #0 {
  entry:
  %fdp3 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il4 = load i8, i8* %fdp3, align 1
  %t2 = zext i8 %il4 to i64
  %t0 = xor i64 %t2, 1
  %ri5 = icmp ne i64 %t0, 0
  ret i1 %ri5
}
define void @async_body_compute(%State* noalias nocapture %state) local_unnamed_addr #0 {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t1 = load i64, i64* %fdp2, align 8
  %fdp4 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %t3 = load i64, i64* %fdp4, align 8
  %c5 = icmp slt i64 %t1, %t3
  %t6 = zext i1 %c5 to i64
  %ri7 = icmp ne i64 %t6, 0
  br i1 %ri7, label %txn_fire_9, label %async_body_compute_done
txn_fire_9:
%ff10 = bitcast i32 1077936128 to float
%fi11 = bitcast float %ff10 to i32
%t9 = zext i32 %fi11 to i64
  ; let x = %t9
%ff14 = bitcast i32 1082130432 to float
%fi15 = bitcast float %ff14 to i32
%t13 = zext i32 %fi15 to i64
  ; let y = %t13
  %bfr20 = fmul fast float %ff10, %ff10
  %bfr24 = fmul fast float %ff14, %ff14
  %bfr25 = fadd fast float %bfr20, %bfr24
  ; let dsq = %bfr25
  %t26 = call float @llvm.sqrt.f32(float %bfr25)
  ; let d = %t26
  %t28 = call i64 @__print_float(float %t26)
  ; let _p = %t28
  %fdp32 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t31 = load i64, i64* %fdp32, align 8
%t34 = add i64 0, 1
  %t35 = add i64 %t31, %t34
  %ap36 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  store i64 %t35, i64* %ap36, align 8
  ret void
async_body_compute_done:
  ret void
}

define void @async_body___init(%State* noalias nocapture %state) local_unnamed_addr #0 {
  entry:
  %fdp3 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il4 = load i8, i8* %fdp3, align 1
  %t2 = zext i8 %il4 to i64
  %t0 = xor i64 %t2, 1
  %ri5 = icmp ne i64 %t0, 0
  br i1 %ri5, label %txn_fire_7, label %async_body___init_done
txn_fire_7:
%ff10 = bitcast i32 1073741824 to float
%fi11 = bitcast float %ff10 to i32
%t9 = zext i32 %fi11 to i64
  %t7 = call float @llvm.sqrt.f32(float %ff10)
  %t6 = call i64 @__print_float(float %t7)
  ret void
  %t12 = add i64 0, 1
  %ap13 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %tr14 = trunc i64 %t12 to i8
  store i8 %tr14, i8* %ap13, align 1
  ret void
async_body___init_done:
  ret void
}

define void @init_state(%State* noalias nocapture %state) local_unnamed_addr #0 {
  entry:
  %ip0 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store i8 0, i8* %ip0, align 1
  %ip1 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
%t1 = add i64 0, 0
  store i64 %t1, i64* %ip1, align 8
  %ip2 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
%sp5 = getelementptr inbounds [6 x i8], [6 x i8]* @str.0, i64 0, i64 0
%t4 = ptrtoint i8* %sp5 to i64
  %fp6 = inttoptr i64 %t4 to i8*
  %t2 = call i64 @__get_env_int(i8* %fp6)
  store i64 %t2, i64* %ip2, align 8
  ret void
}

define void @reactor_tick(%State* noalias nocapture %state) local_unnamed_addr #2 {
  entry:
  %fired_mask = alloca i64, align 8
  store i64 0, i64* %fired_mask
  %pr0 = call i1 @pre_compute(%State* %state)
  %pr1 = call i1 @pre___init(%State* %state)
  br i1 %pr0, label %b0, label %ck1
ck1:
  %fm1 = load i64, i64* %fired_mask
  %ca1 = and i64 %fm1, 1
  %nc1 = icmp eq i64 %ca1, 0
  %can1 = and i1 %pr1, %nc1
  br i1 %can1, label %b1, label %ck2
b0:
  call void @compute(%State* %state)
  %fm0a = load i64, i64* %fired_mask
  %fm0b = or i64 %fm0a, 2
  store i64 %fm0b, i64* %fired_mask
  br label %ck1
b1:
  call void @__init(%State* %state)
  %fm1a = load i64, i64* %fired_mask
  %fm1b = or i64 %fm1a, 1
  store i64 %fm1b, i64* %fired_mask
  br label %ck2
ck2:
  ret void
}

define i32 @main() local_unnamed_addr #3 {
  entry:
  %state = alloca %State, align 8
  call void @init_state(%State* noalias nocapture %state)
  %tp_fn_ptr = bitcast [2 x void (%State*)*]* @thread_pool_fns to i8**
  call void @briev_thread_pool_init(i32 2, i8** %tp_fn_ptr)
  br label %tick
  tick:
  call void @briev_barrier_release()
  call void @reactor_tick(%State* noalias nocapture %state)
  call void @briev_barrier_wait()
  %gep_exit_9 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t8 = load i64, i64* %gep_exit_9, align 8
  %gep_exit_11 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %t10 = load i64, i64* %gep_exit_11, align 8
  %t12 = icmp eq i64 %t8, %t10
  %t7 = zext i1 %t12 to i64
  %t13 = trunc i64 %t7 to i1
  br i1 %t13, label %done, label %tick
  done:
  ret i32 0
}

@llvm.thread_pool = constant [2 x i8*] [i8* bitcast (void (%State*)* @async_body_compute to i8*), i8* bitcast (void (%State*)* @async_body___init to i8*)]
@thread_pool_fns = private constant [2 x void (%State*)*] [void (%State*)* @async_body_compute, void (%State*)* @async_body___init]

attributes #0 = {
    mustprogress nofree norecurse nosync nounwind willreturn
    memory(argmem: readwrite)
}
attributes #1 = { nocallback nofree nosync nounwind willreturn }
attributes #2 = { mustprogress nofree norecurse nosync nounwind memory(readwrite) }
attributes #3 = { nofree norecurse nosync nounwind memory(readwrite) }
