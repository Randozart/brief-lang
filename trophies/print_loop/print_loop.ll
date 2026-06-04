; ModuleID = 'program.ll'
source_filename = "program.bv"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

declare void @llvm.assume(i1) #1
declare void @__rt_init() local_unnamed_addr
declare void @__rt_poll() local_unnamed_addr
declare void @__rt_wait() local_unnamed_addr
declare void @brief_thread_pool_init(i32, i8**) local_unnamed_addr
declare void @brief_barrier_release() local_unnamed_addr
declare void @brief_barrier_wait() local_unnamed_addr
declare i64 @__print_int(i64) #1
@__io_pending = external global i8, align 1

@N = constant i64 50000000
@print_interval = constant i64 100000

%State = type { i64 }
; %State is allocated on the stack in main() as %state = alloca %State

define void @work(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %tr2 = load volatile i8, i8* @__io_pending
  %tz3 = zext i8 %tr2 to i64
  %t1 = add i64 0, %tz3
  %fdp6 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il7 = load i64, i64* %fdp6, align 8
  %t5 = add i64 0, %il7
  %t8 = add i64 0, 50000000
  %c9 = icmp slt i64 %t5, %t8
  %t4 = zext i1 %c9 to i64
  %t0 = and i64 %t1, %t4
  %pi10 = icmp ne i64 %t0, 0
  br i1 %pi10, label %ps12, label %pp11
  pp11:
    unreachable
  ps12:
  call void @llvm.assume(i1 %pi10)
  %fdp15 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il16 = load i64, i64* %fdp15, align 8
  %t14 = add i64 0, %il16
  %t17 = add i64 0, 1
  %t13 = add i64 %t14, %t17
  %ap18 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store i64 %t13, i64* %ap18, align 8
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
    %t29 = call i64 @__print_int(i64 %t30)
    br label %g28_e
  g28_e:
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
  %t8 = add i64 0, 50000000
  %c9 = icmp slt i64 %t5, %t8
  %t4 = zext i1 %c9 to i64
  %t0 = and i64 %t1, %t4
  %ri10 = icmp ne i64 %t0, 0
  ret i1 %ri10
}
define void @init_state(%State* noalias nocapture %state) local_unnamed_addr #0 {
  entry:
  %ip0 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store i64 0, i64* %ip0, align 8
  ret void
}

define void @reactor_tick(%State* noalias nocapture %state) local_unnamed_addr #2 {
  entry:
  %tr0 = load volatile i8, i8* @__io_pending
  %tz1 = zext i8 %tr0 to i64
  %sz_io_pending = add i64 0, %tz1
  %fired_mask = alloca i64, align 8
  store i64 0, i64* %fired_mask
  %pr0 = call i1 @pre_work(%State* %state)
  br i1 %pr0, label %b0, label %ck1
b0:
  call void @work(%State* %state)
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
  call void @__rt_init()
  call void @__rt_poll()
  br label %tick
tick:
  %tr2 = load volatile i8, i8* @__io_pending
  %tz3 = zext i8 %tr2 to i64
  %sz_io_pending = add i64 0, %tz3
  %ltuni_work_4 = load i64, i64* @N, align 8
  br label %uni_work_pre
uni_work_pre:
  %iivuni_work_89 = insertvalue %State zeroinitializer, i64 0, 0
  %slot_uni_work = alloca %State, align 8
  store %State %iivuni_work_89, %State* %slot_uni_work, align 8
  br label %uni_work_hdr
uni_work_hdr:
  %ssa_phi_uni_work = load %State, %State* %slot_uni_work, align 8
  %exuni_work_90 = extractvalue %State %ssa_phi_uni_work, 0
  %adjuni_work_91 = add i64 %ltuni_work_4, -3
  %cpuni_work_92 = icmp slt i64 %exuni_work_90, %adjuni_work_91
  br i1 %cpuni_work_92, label %uni_work_body4, label %uni_work_rem
uni_work_rem:
  %cpuni_work_93 = icmp slt i64 %exuni_work_90, %ltuni_work_4
  br i1 %cpuni_work_93, label %uni_work_body1, label %uni_work_done
uni_work_body4:
  %ev6 = extractvalue %State %ssa_phi_uni_work, 0
  %t5 = add i64 0, %ev6
  %t7 = add i64 0, 1
  %t4 = add i64 %t5, %t7
  %in8 = insertvalue %State %ssa_phi_uni_work, i64 %t4, 0
  %ev12 = extractvalue %State %in8, 0
  %t11 = add i64 0, %ev12
  %t13 = add i64 0, 100000
  %t10 = srem i64 %t11, %t13
  %t14 = add i64 0, 0
  %c15 = icmp eq i64 %t10, %t14
  %t9 = zext i1 %c15 to i64
  %gc16 = icmp ne i64 %t9, 0
  br i1 %gc16, label %g17_t, label %g17_e
  g17_t:
    %ev20 = extractvalue %State %in8, 0
    %t19 = add i64 0, %ev20
    %t18 = call i64 @__print_int(i64 %t19)
    br label %g17_e
  g17_e:
  %ev23 = extractvalue %State %in8, 0
  %t22 = add i64 0, %ev23
  %t24 = add i64 0, 1
  %t21 = add i64 %t22, %t24
  %in25 = insertvalue %State %in8, i64 %t21, 0
  %ev29 = extractvalue %State %in25, 0
  %t28 = add i64 0, %ev29
  %t30 = add i64 0, 100000
  %t27 = srem i64 %t28, %t30
  %t31 = add i64 0, 0
  %c32 = icmp eq i64 %t27, %t31
  %t26 = zext i1 %c32 to i64
  %gc33 = icmp ne i64 %t26, 0
  br i1 %gc33, label %g34_t, label %g34_e
  g34_t:
    %ev37 = extractvalue %State %in25, 0
    %t36 = add i64 0, %ev37
    %t35 = call i64 @__print_int(i64 %t36)
    br label %g34_e
  g34_e:
  %ev40 = extractvalue %State %in25, 0
  %t39 = add i64 0, %ev40
  %t41 = add i64 0, 1
  %t38 = add i64 %t39, %t41
  %in42 = insertvalue %State %in25, i64 %t38, 0
  %ev46 = extractvalue %State %in42, 0
  %t45 = add i64 0, %ev46
  %t47 = add i64 0, 100000
  %t44 = srem i64 %t45, %t47
  %t48 = add i64 0, 0
  %c49 = icmp eq i64 %t44, %t48
  %t43 = zext i1 %c49 to i64
  %gc50 = icmp ne i64 %t43, 0
  br i1 %gc50, label %g51_t, label %g51_e
  g51_t:
    %ev54 = extractvalue %State %in42, 0
    %t53 = add i64 0, %ev54
    %t52 = call i64 @__print_int(i64 %t53)
    br label %g51_e
  g51_e:
  %ev57 = extractvalue %State %in42, 0
  %t56 = add i64 0, %ev57
  %t58 = add i64 0, 1
  %t55 = add i64 %t56, %t58
  %in59 = insertvalue %State %in42, i64 %t55, 0
  %ev63 = extractvalue %State %in59, 0
  %t62 = add i64 0, %ev63
  %t64 = add i64 0, 100000
  %t61 = srem i64 %t62, %t64
  %t65 = add i64 0, 0
  %c66 = icmp eq i64 %t61, %t65
  %t60 = zext i1 %c66 to i64
  %gc67 = icmp ne i64 %t60, 0
  br i1 %gc67, label %g68_t, label %g68_e
  g68_t:
    %ev71 = extractvalue %State %in59, 0
    %t70 = add i64 0, %ev71
    %t69 = call i64 @__print_int(i64 %t70)
    br label %g68_e
  g68_e:
  store %State %in59, %State* %slot_uni_work, align 8
  br label %uni_work_hdr
uni_work_body1:
  %ev74 = extractvalue %State %ssa_phi_uni_work, 0
  %t73 = add i64 0, %ev74
  %t75 = add i64 0, 1
  %t72 = add i64 %t73, %t75
  %in76 = insertvalue %State %ssa_phi_uni_work, i64 %t72, 0
  %ev80 = extractvalue %State %in76, 0
  %t79 = add i64 0, %ev80
  %t81 = add i64 0, 100000
  %t78 = srem i64 %t79, %t81
  %t82 = add i64 0, 0
  %c83 = icmp eq i64 %t78, %t82
  %t77 = zext i1 %c83 to i64
  %gc84 = icmp ne i64 %t77, 0
  br i1 %gc84, label %g85_t, label %g85_e
  g85_t:
    %ev88 = extractvalue %State %in76, 0
    %t87 = add i64 0, %ev88
    %t86 = call i64 @__print_int(i64 %t87)
    br label %g85_e
  g85_e:
  store %State %in76, %State* %slot_uni_work, align 8
  br label %uni_work_hdr
uni_work_done:
  %final_uni_work = load %State, %State* %slot_uni_work, align 8
  store %State %final_uni_work, %State* %state, align 8
  br label %exit_check
io_pending_residual:
  call void @reactor_tick(%State* noalias nocapture %state)
  br label %exit_check
exit_check:
  %gep_exit_96 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t95 = load i64, i64* %gep_exit_96, align 8
  %t97 = load i64, i64* @N, align 8
  %t98 = icmp eq i64 %t95, %t97
  %t94 = zext i1 %t98 to i64
  %t99 = trunc i64 %t94 to i1
  br i1 %t99, label %done, label %do_wait
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
