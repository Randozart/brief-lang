; ModuleID = 'program.ll'
source_filename = "program.bv"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

declare void @llvm.assume(i1) #1
declare i64 @llvm.ctpop.i64(i64) #1
declare i64 @llvm.ctlz.i64(i64, i1) #1
declare i64 @llvm.cttz.i64(i64, i1) #1
declare i64 @llvm.abs.i64(i64, i1) #1
declare double @llvm.fabs.f64(double) #1
declare i64 @llvm.bitreverse.i64(i64) #1
declare void @__rt_init() local_unnamed_addr
declare void @__rt_poll() local_unnamed_addr
declare void @__rt_wait() local_unnamed_addr
declare void @briv_thread_pool_init(i32, i8**) local_unnamed_addr
declare void @briv_barrier_release() local_unnamed_addr
declare void @briv_barrier_wait() local_unnamed_addr
declare void @__exit(i64) local_unnamed_addr
declare i64 @__print_int(i64) #6
declare i64 @__get_env_int(i8*) #1
@R1 = constant i64 200
@R2 = constant i64 199

%State = type { i64, i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant [6 x i8] c"BOUND\00", align 1

define void @step(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il3 = load i64, i64* %fdp2, align 8
  %t1 = add i64 0, %il3
  %fdp5 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il6 = load i64, i64* %fdp5, align 8
  %t4 = add i64 0, %il6
  %c7 = icmp slt i64 %t1, %t4
  %t0 = zext i1 %c7 to i64
  %pi8 = icmp ne i64 %t0, 0
  br i1 %pi8, label %ps10, label %pp9
  pp9:
    unreachable
  ps10:
  call void @llvm.assume(i1 %pi8)
  %fdp13 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il14 = load i64, i64* %fdp13, align 8
  %t12 = add i64 0, %il14
  %fdp16 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il17 = load i64, i64* %fdp16, align 8
  %t15 = add i64 0, %il17
  %t11 = add i64 %t12, %t15
  %ap18 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  store i64 %t11, i64* %ap18, align 8
  %fdp22 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il23 = load i64, i64* %fdp22, align 8
  %t21 = add i64 0, %il23
  %t24 = add i64 0, 200
  %t20 = add i64 %t21, %t24
  %t25 = add i64 0, 199
  %t19 = sub i64 %t20, %t25
  %ap26 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  store i64 %t19, i64* %ap26, align 8
  %fdp30 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il31 = load i64, i64* %fdp30, align 8
  %t29 = add i64 0, %il31
  %t32 = add i64 0, 5000000
  %t28 = srem i64 %t29, %t32
  %t33 = add i64 0, 0
  %c34 = icmp eq i64 %t28, %t33
  %t27 = zext i1 %c34 to i64
  %gc35 = icmp ne i64 %t27, 0
  br i1 %gc35, label %g36_t, label %g36_e
  g36_t:
    %fdp39 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
    %il40 = load i64, i64* %fdp39, align 8
    %t38 = add i64 0, %il40
    %t37 = call i64 @__print_int(i64 %t38) #6
    br label %g36_e
  g36_e:
  ret void
}

define internal i1 @pre_step(%State* noalias nocapture %state) #0 {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il3 = load i64, i64* %fdp2, align 8
  %t1 = add i64 0, %il3
  %fdp5 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il6 = load i64, i64* %fdp5, align 8
  %t4 = add i64 0, %il6
  %c7 = icmp slt i64 %t1, %t4
  %t0 = zext i1 %c7 to i64
  %ri8 = icmp ne i64 %t0, 0
  ret i1 %ri8
}
define void @init_state(%State* noalias nocapture %state) local_unnamed_addr #0 {
  entry:
  %ip0 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %sp2 = getelementptr inbounds [6 x i8], [6 x i8]* @str.0, i64 0, i64 0
  %t1 = ptrtoint i8* %sp2 to i64
  %fp3 = inttoptr i64 %t1 to i8*
  %t0 = call i64 @__get_env_int(i8* %fp3)
  store i64 %t0, i64* %ip0, align 8
  %ip1 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  store i64 0, i64* %ip1, align 8
  %ip2 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  store i64 0, i64* %ip2, align 8
  ret void
}

define void @reactor_tick(%State* noalias nocapture %state) local_unnamed_addr #2 {
  entry:
  %fired_mask = alloca i64, align 8
  store i64 0, i64* %fired_mask
  %pr0 = call i1 @pre_step(%State* %state)
  br i1 %pr0, label %b0, label %ck1
b0:
  call void @step(%State* %state)
  %fm0a = load i64, i64* %fired_mask
  %fm0b = or i64 %fm0a, 6
  store i64 %fm0b, i64* %fired_mask
  br label %ck1
ck1:
  ret void
}

define i32 @main() local_unnamed_addr #3 {
  entry:
  %state = alloca %State, align 8
  call void @init_state(%State* noalias nocapture %state)
  br label %tick
  tick:
  call void @reactor_tick(%State* noalias nocapture %state)
  %gep_exit_7 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %t6 = load i64, i64* %gep_exit_7, align 8
  %gep_exit_9 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t8 = load i64, i64* %gep_exit_9, align 8
  %t10 = icmp eq i64 %t6, %t8
  %t5 = zext i1 %t10 to i64
  %gep_exit_13 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %t12 = load i64, i64* %gep_exit_13, align 8
  %t14 = add i64 0, 0
  %t15 = icmp sge i64 %t12, %t14
  %t11 = zext i1 %t15 to i64
  %t4 = and i64 %t5, %t11
  %t16 = trunc i64 %t4 to i1
  br i1 %t16, label %done, label %tick
  done:
  ret i32 0
}


attributes #0 = {
    mustprogress nofree norecurse nosync nounwind willreturn
    memory(argmem: readwrite)
}
attributes #1 = { nocallback nofree nosync nounwind willreturn }
attributes #6 = { nocallback nofree nosync nounwind willreturn memory(write) }
attributes #2 = { mustprogress nofree norecurse nosync nounwind memory(readwrite) }
attributes #3 = { nofree norecurse nosync nounwind memory(readwrite) }
