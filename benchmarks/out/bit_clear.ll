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
@initial_reg = constant i64 9223372036854775807

%State = type { i64 }
; %State is allocated on the stack in main() as %state = alloca %State

define void @clear(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il3 = load i64, i64* %fdp2, align 8
  %t1 = add i64 0, %il3
  %t4 = add i64 0, 0
  %c5 = icmp ne i64 %t1, %t4
  %t0 = zext i1 %c5 to i64
  %pi6 = icmp ne i64 %t0, 0
  br i1 %pi6, label %ps8, label %pp7
  pp7:
    unreachable
  ps8:
  call void @llvm.assume(i1 %pi6)
  %fdp11 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il12 = load i64, i64* %fdp11, align 8
  %t10 = add i64 0, %il12
  %fdp15 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il16 = load i64, i64* %fdp15, align 8
  %t14 = add i64 0, %il16
  %t17 = add i64 0, 1
  %t13 = sub i64 %t14, %t17
  %t9 = and i64 %t10, %t13
  %ap18 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store i64 %t9, i64* %ap18, align 8
  %fdp22 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il23 = load i64, i64* %fdp22, align 8
  %t21 = add i64 0, %il23
  %t24 = add i64 0, 100000
  %t20 = srem i64 %t21, %t24
  %t25 = add i64 0, 0
  %c26 = icmp eq i64 %t20, %t25
  %t19 = zext i1 %c26 to i64
  %gc27 = icmp ne i64 %t19, 0
  br i1 %gc27, label %g28_t, label %g28_e
  g28_t:
    %fdp31 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
    %il32 = load i64, i64* %fdp31, align 8
    %t30 = add i64 0, %il32
    %t29 = call i64 @__print_int(i64 %t30) #6
    br label %g28_e
  g28_e:
  ret void
}

define internal i1 @pre_clear(%State* noalias nocapture %state) #0 {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il3 = load i64, i64* %fdp2, align 8
  %t1 = add i64 0, %il3
  %t4 = add i64 0, 0
  %c5 = icmp ne i64 %t1, %t4
  %t0 = zext i1 %c5 to i64
  %ri6 = icmp ne i64 %t0, 0
  ret i1 %ri6
}
define void @init_state(%State* noalias nocapture %state) local_unnamed_addr #0 {
  entry:
  %ip0 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t0 = add i64 0, 9223372036854775807
  store i64 %t0, i64* %ip0, align 8
  ret void
}

define void @reactor_tick(%State* noalias nocapture %state) local_unnamed_addr #2 {
  entry:
  %fired_mask = alloca i64, align 8
  store i64 0, i64* %fired_mask
  %pr0 = call i1 @pre_clear(%State* %state)
  br i1 %pr0, label %b0, label %ck1
b0:
  call void @clear(%State* %state)
  %fm0a = load i64, i64* %fired_mask
  %fm0b = or i64 %fm0a, 1
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
  %gep_exit_3 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t2 = load i64, i64* %gep_exit_3, align 8
  %t4 = add i64 0, 0
  %t5 = icmp eq i64 %t2, %t4
  %t1 = zext i1 %t5 to i64
  %t6 = trunc i64 %t1 to i1
  br i1 %t6, label %done, label %tick
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
