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
declare void @brief_thread_pool_init(i32, i8**) local_unnamed_addr
declare void @brief_barrier_release() local_unnamed_addr
declare void @brief_barrier_wait() local_unnamed_addr
declare void @__exit(i64) local_unnamed_addr
declare i64 @__print_int(i64) #6
declare i64 @__get_env_int(i8*) #1
@R = constant i64 100

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
  %fdp21 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il22 = load i64, i64* %fdp21, align 8
  %t20 = add i64 0, %il22
  %t23 = add i64 0, 1
  %t19 = add i64 %t20, %t23
  %ap24 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  store i64 %t19, i64* %ap24, align 8
  %fdp28 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il29 = load i64, i64* %fdp28, align 8
  %t27 = add i64 0, %il29
  %t30 = add i64 0, 5000000
  %t26 = srem i64 %t27, %t30
  %t31 = add i64 0, 0
  %c32 = icmp eq i64 %t26, %t31
  %t25 = zext i1 %c32 to i64
  %gc33 = icmp ne i64 %t25, 0
  br i1 %gc33, label %g34_t, label %g34_e
  g34_t:
    %fdp37 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
    %il38 = load i64, i64* %fdp37, align 8
    %t36 = add i64 0, %il38
    %t35 = call i64 @__print_int(i64 %t36) #6
    br label %g34_e
  g34_e:
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

define i32 @main() local_unnamed_addr #0 {
  entry:
  %state = alloca %State, align 8
  call void @init_state(%State* noalias nocapture %state)
  %gtcase_4 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %ltcase_4 = load i64, i64* %gtcase_4, align 8
  br label %case_pre
case_pre:
  %gepcase_109 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %ldcase_110 = load i64, i64* %gepcase_109, align 8
  %livcase_111 = insertvalue %State zeroinitializer, i64 %ldcase_110, 0
  %iivcase_112 = insertvalue %State %livcase_111, i64 0, 1
  %iivcase_113 = insertvalue %State %iivcase_112, i64 0, 2
  %slot_case = alloca %State, align 8
  store %State %iivcase_113, %State* %slot_case, align 8
  br label %case_hdr
case_hdr:
  %ssa_phi_case = load %State, %State* %slot_case, align 8
  %excase_114 = extractvalue %State %ssa_phi_case, 1
  %adjcase_115 = add i64 %ltcase_4, -3
  %cpcase_116 = icmp slt i64 %excase_114, %adjcase_115
  br i1 %cpcase_116, label %case_body4, label %case_rem
case_rem:
  %cpcase_117 = icmp slt i64 %excase_114, %ltcase_4
  br i1 %cpcase_117, label %case_body1, label %case_done
case_body4:
  %count_old_4 = extractvalue %State %ssa_phi_case, 1
  %acc_old_5 = extractvalue %State %ssa_phi_case, 2
  %N_old_6 = extractvalue %State %ssa_phi_case, 0
  %t8 = add i64 0, %acc_old_5
  %t9 = add i64 0, %count_old_4
  %t7 = add i64 %t8, %t9
  %in10 = insertvalue %State %ssa_phi_case, i64 %t7, 2
  %t12 = add i64 0, %count_old_4
  %t13 = add i64 0, 1
  %t11 = add i64 %t12, %t13
  %in14 = insertvalue %State %in10, i64 %t11, 1
  %t17 = add i64 0, %count_old_4
  %t18 = add i64 0, 5000000
  %t16 = srem i64 %t17, %t18
  %t19 = add i64 0, 0
  %c20 = icmp eq i64 %t16, %t19
  %t15 = zext i1 %c20 to i64
  %gc21 = icmp ne i64 %t15, 0
  br i1 %gc21, label %g22_t, label %g22_e
  g22_t:
    %t24 = add i64 0, %acc_old_5
    %t23 = call i64 @__print_int(i64 %t24) #6
    br label %g22_e
  g22_e:
  %count_old_25 = extractvalue %State %in14, 1
  %acc_old_26 = extractvalue %State %in14, 2
  %N_old_27 = extractvalue %State %in14, 0
  %t29 = add i64 0, %acc_old_26
  %t30 = add i64 0, %count_old_25
  %t28 = add i64 %t29, %t30
  %in31 = insertvalue %State %in14, i64 %t28, 2
  %t33 = add i64 0, %count_old_25
  %t34 = add i64 0, 1
  %t32 = add i64 %t33, %t34
  %in35 = insertvalue %State %in31, i64 %t32, 1
  %t38 = add i64 0, %count_old_25
  %t39 = add i64 0, 5000000
  %t37 = srem i64 %t38, %t39
  %t40 = add i64 0, 0
  %c41 = icmp eq i64 %t37, %t40
  %t36 = zext i1 %c41 to i64
  %gc42 = icmp ne i64 %t36, 0
  br i1 %gc42, label %g43_t, label %g43_e
  g43_t:
    %t45 = add i64 0, %acc_old_26
    %t44 = call i64 @__print_int(i64 %t45) #6
    br label %g43_e
  g43_e:
  %count_old_46 = extractvalue %State %in35, 1
  %acc_old_47 = extractvalue %State %in35, 2
  %N_old_48 = extractvalue %State %in35, 0
  %t50 = add i64 0, %acc_old_47
  %t51 = add i64 0, %count_old_46
  %t49 = add i64 %t50, %t51
  %in52 = insertvalue %State %in35, i64 %t49, 2
  %t54 = add i64 0, %count_old_46
  %t55 = add i64 0, 1
  %t53 = add i64 %t54, %t55
  %in56 = insertvalue %State %in52, i64 %t53, 1
  %t59 = add i64 0, %count_old_46
  %t60 = add i64 0, 5000000
  %t58 = srem i64 %t59, %t60
  %t61 = add i64 0, 0
  %c62 = icmp eq i64 %t58, %t61
  %t57 = zext i1 %c62 to i64
  %gc63 = icmp ne i64 %t57, 0
  br i1 %gc63, label %g64_t, label %g64_e
  g64_t:
    %t66 = add i64 0, %acc_old_47
    %t65 = call i64 @__print_int(i64 %t66) #6
    br label %g64_e
  g64_e:
  %count_old_67 = extractvalue %State %in56, 1
  %acc_old_68 = extractvalue %State %in56, 2
  %N_old_69 = extractvalue %State %in56, 0
  %t71 = add i64 0, %acc_old_68
  %t72 = add i64 0, %count_old_67
  %t70 = add i64 %t71, %t72
  %in73 = insertvalue %State %in56, i64 %t70, 2
  %t75 = add i64 0, %count_old_67
  %t76 = add i64 0, 1
  %t74 = add i64 %t75, %t76
  %in77 = insertvalue %State %in73, i64 %t74, 1
  %t80 = add i64 0, %count_old_67
  %t81 = add i64 0, 5000000
  %t79 = srem i64 %t80, %t81
  %t82 = add i64 0, 0
  %c83 = icmp eq i64 %t79, %t82
  %t78 = zext i1 %c83 to i64
  %gc84 = icmp ne i64 %t78, 0
  br i1 %gc84, label %g85_t, label %g85_e
  g85_t:
    %t87 = add i64 0, %acc_old_68
    %t86 = call i64 @__print_int(i64 %t87) #6
    br label %g85_e
  g85_e:
  store %State %in77, %State* %slot_case, align 8
  br label %case_hdr
case_body1:
  %count_old_88 = extractvalue %State %ssa_phi_case, 1
  %acc_old_89 = extractvalue %State %ssa_phi_case, 2
  %N_old_90 = extractvalue %State %ssa_phi_case, 0
  %t92 = add i64 0, %acc_old_89
  %t93 = add i64 0, %count_old_88
  %t91 = add i64 %t92, %t93
  %in94 = insertvalue %State %ssa_phi_case, i64 %t91, 2
  %t96 = add i64 0, %count_old_88
  %t97 = add i64 0, 1
  %t95 = add i64 %t96, %t97
  %in98 = insertvalue %State %in94, i64 %t95, 1
  %t101 = add i64 0, %count_old_88
  %t102 = add i64 0, 5000000
  %t100 = srem i64 %t101, %t102
  %t103 = add i64 0, 0
  %c104 = icmp eq i64 %t100, %t103
  %t99 = zext i1 %c104 to i64
  %gc105 = icmp ne i64 %t99, 0
  br i1 %gc105, label %g106_t, label %g106_e
  g106_t:
    %t108 = add i64 0, %acc_old_89
    %t107 = call i64 @__print_int(i64 %t108) #6
    br label %g106_e
  g106_e:
  store %State %in98, %State* %slot_case, align 8
  br label %case_hdr
case_done:
  %final_case = load %State, %State* %slot_case, align 8
  store %State %final_case, %State* %state, align 8
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
