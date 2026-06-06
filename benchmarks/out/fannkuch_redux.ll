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
declare i64 @__get_env_int(i8*) #1
declare i64 @__print_int(i64) #6
@IA = constant i64 3877
@IM = constant i64 139968
@IC = constant i64 29573

%State = type { i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant [6 x i8] c"BOUND\00", align 1

define void @fan(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il3 = load i64, i64* %fdp2, align 8
  %t1 = add i64 0, %il3
  %fdp5 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
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
  %t15 = add i64 0, 3877
  %t11 = mul i64 %t12, %t15
  %ap16 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  store i64 %t11, i64* %ap16, align 8
  %fdp19 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il20 = load i64, i64* %fdp19, align 8
  %t18 = add i64 0, %il20
  %t21 = add i64 0, 29573
  %t17 = add i64 %t18, %t21
  %ap22 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  store i64 %t17, i64* %ap22, align 8
  %fdp25 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il26 = load i64, i64* %fdp25, align 8
  %t24 = add i64 0, %il26
  %t27 = add i64 0, 139968
  %t23 = srem i64 %t24, %t27
  %ap28 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  store i64 %t23, i64* %ap28, align 8
  %fdp30 = getelementptr inbounds %State, %State* %state, i32 0, i32 5
  %il31 = load i64, i64* %fdp30, align 8
  %t29 = add i64 0, %il31
  %ap32 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  store i64 %t29, i64* %ap32, align 8
  %fdp34 = getelementptr inbounds %State, %State* %state, i32 0, i32 6
  %il35 = load i64, i64* %fdp34, align 8
  %t33 = add i64 0, %il35
  %ap36 = getelementptr inbounds %State, %State* %state, i32 0, i32 5
  store i64 %t33, i64* %ap36, align 8
  %fdp38 = getelementptr inbounds %State, %State* %state, i32 0, i32 7
  %il39 = load i64, i64* %fdp38, align 8
  %t37 = add i64 0, %il39
  %ap40 = getelementptr inbounds %State, %State* %state, i32 0, i32 6
  store i64 %t37, i64* %ap40, align 8
  %fdp42 = getelementptr inbounds %State, %State* %state, i32 0, i32 8
  %il43 = load i64, i64* %fdp42, align 8
  %t41 = add i64 0, %il43
  %ap44 = getelementptr inbounds %State, %State* %state, i32 0, i32 7
  store i64 %t41, i64* %ap44, align 8
  %fdp46 = getelementptr inbounds %State, %State* %state, i32 0, i32 9
  %il47 = load i64, i64* %fdp46, align 8
  %t45 = add i64 0, %il47
  %ap48 = getelementptr inbounds %State, %State* %state, i32 0, i32 8
  store i64 %t45, i64* %ap48, align 8
  %fdp50 = getelementptr inbounds %State, %State* %state, i32 0, i32 10
  %il51 = load i64, i64* %fdp50, align 8
  %t49 = add i64 0, %il51
  %ap52 = getelementptr inbounds %State, %State* %state, i32 0, i32 9
  store i64 %t49, i64* %ap52, align 8
  %fdp54 = getelementptr inbounds %State, %State* %state, i32 0, i32 11
  %il55 = load i64, i64* %fdp54, align 8
  %t53 = add i64 0, %il55
  %ap56 = getelementptr inbounds %State, %State* %state, i32 0, i32 10
  store i64 %t53, i64* %ap56, align 8
  %fdp58 = getelementptr inbounds %State, %State* %state, i32 0, i32 12
  %il59 = load i64, i64* %fdp58, align 8
  %t57 = add i64 0, %il59
  %ap60 = getelementptr inbounds %State, %State* %state, i32 0, i32 11
  store i64 %t57, i64* %ap60, align 8
  %fdp62 = getelementptr inbounds %State, %State* %state, i32 0, i32 13
  %il63 = load i64, i64* %fdp62, align 8
  %t61 = add i64 0, %il63
  %ap64 = getelementptr inbounds %State, %State* %state, i32 0, i32 12
  store i64 %t61, i64* %ap64, align 8
  %fdp66 = getelementptr inbounds %State, %State* %state, i32 0, i32 14
  %il67 = load i64, i64* %fdp66, align 8
  %t65 = add i64 0, %il67
  %ap68 = getelementptr inbounds %State, %State* %state, i32 0, i32 13
  store i64 %t65, i64* %ap68, align 8
  %fdp70 = getelementptr inbounds %State, %State* %state, i32 0, i32 15
  %il71 = load i64, i64* %fdp70, align 8
  %t69 = add i64 0, %il71
  %ap72 = getelementptr inbounds %State, %State* %state, i32 0, i32 14
  store i64 %t69, i64* %ap72, align 8
  %fdp74 = getelementptr inbounds %State, %State* %state, i32 0, i32 16
  %il75 = load i64, i64* %fdp74, align 8
  %t73 = add i64 0, %il75
  %ap76 = getelementptr inbounds %State, %State* %state, i32 0, i32 15
  store i64 %t73, i64* %ap76, align 8
  %fdp78 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il79 = load i64, i64* %fdp78, align 8
  %t77 = add i64 0, %il79
  %ap80 = getelementptr inbounds %State, %State* %state, i32 0, i32 16
  store i64 %t77, i64* %ap80, align 8
  %fdp83 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  %il84 = load i64, i64* %fdp83, align 8
  %t82 = add i64 0, %il84
  %fdp87 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il88 = load i64, i64* %fdp87, align 8
  %t86 = add i64 0, %il88
  %t89 = add i64 0, 13
  %t85 = srem i64 %t86, %t89
  %t81 = add i64 %t82, %t85
  %ap90 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  store i64 %t81, i64* %ap90, align 8
  %fdp93 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  %il94 = load i64, i64* %fdp93, align 8
  %t92 = add i64 0, %il94
  %fdp97 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  %il98 = load i64, i64* %fdp97, align 8
  %t96 = add i64 0, %il98
  %t99 = add i64 0, 17
  %t95 = srem i64 %t96, %t99
  %t91 = add i64 %t92, %t95
  %ap100 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  store i64 %t91, i64* %ap100, align 8
  %fdp103 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il104 = load i64, i64* %fdp103, align 8
  %t102 = add i64 0, %il104
  %t105 = add i64 0, 1
  %t101 = add i64 %t102, %t105
  %ap106 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store i64 %t101, i64* %ap106, align 8
  %fdp109 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il110 = load i64, i64* %fdp109, align 8
  %t108 = add i64 0, %il110
  %fdp112 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il113 = load i64, i64* %fdp112, align 8
  %t111 = add i64 0, %il113
  %c114 = icmp eq i64 %t108, %t111
  %t107 = zext i1 %c114 to i64
  %gc115 = icmp ne i64 %t107, 0
  br i1 %gc115, label %g116_t, label %g116_e
  g116_t:
    %fdp119 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
    %il120 = load i64, i64* %fdp119, align 8
    %t118 = add i64 0, %il120
    %t117 = call i64 @__print_int(i64 %t118) #6
    ret void
  g116_e:
  ret void
}

define internal i1 @pre_fan(%State* noalias nocapture %state) #0 {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il3 = load i64, i64* %fdp2, align 8
  %t1 = add i64 0, %il3
  %fdp5 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
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
  store i64 0, i64* %ip0, align 8
  %ip1 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %sp2 = getelementptr inbounds [6 x i8], [6 x i8]* @str.0, i64 0, i64 0
  %t1 = ptrtoint i8* %sp2 to i64
  %fp3 = inttoptr i64 %t1 to i8*
  %t0 = call i64 @__get_env_int(i8* %fp3)
  store i64 %t0, i64* %ip1, align 8
  %ip2 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  store i64 42, i64* %ip2, align 8
  %ip3 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  store i64 0, i64* %ip3, align 8
  %ip4 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  store i64 0, i64* %ip4, align 8
  %ip5 = getelementptr inbounds %State, %State* %state, i32 0, i32 5
  store i64 0, i64* %ip5, align 8
  %ip6 = getelementptr inbounds %State, %State* %state, i32 0, i32 6
  store i64 1, i64* %ip6, align 8
  %ip7 = getelementptr inbounds %State, %State* %state, i32 0, i32 7
  store i64 2, i64* %ip7, align 8
  %ip8 = getelementptr inbounds %State, %State* %state, i32 0, i32 8
  store i64 3, i64* %ip8, align 8
  %ip9 = getelementptr inbounds %State, %State* %state, i32 0, i32 9
  store i64 4, i64* %ip9, align 8
  %ip10 = getelementptr inbounds %State, %State* %state, i32 0, i32 10
  store i64 5, i64* %ip10, align 8
  %ip11 = getelementptr inbounds %State, %State* %state, i32 0, i32 11
  store i64 6, i64* %ip11, align 8
  %ip12 = getelementptr inbounds %State, %State* %state, i32 0, i32 12
  store i64 7, i64* %ip12, align 8
  %ip13 = getelementptr inbounds %State, %State* %state, i32 0, i32 13
  store i64 8, i64* %ip13, align 8
  %ip14 = getelementptr inbounds %State, %State* %state, i32 0, i32 14
  store i64 9, i64* %ip14, align 8
  %ip15 = getelementptr inbounds %State, %State* %state, i32 0, i32 15
  store i64 10, i64* %ip15, align 8
  %ip16 = getelementptr inbounds %State, %State* %state, i32 0, i32 16
  store i64 11, i64* %ip16, align 8
  ret void
}

define i32 @main() local_unnamed_addr #0 {
  entry:
  %state = alloca %State, align 8
  call void @init_state(%State* noalias nocapture %state)
  %gtcase_4 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %ltcase_4 = load i64, i64* %gtcase_4, align 8
  br label %case_pre
case_pre:
  %iivcase_399 = insertvalue %State zeroinitializer, i64 0, 0
  %gepcase_400 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %ldcase_401 = load i64, i64* %gepcase_400, align 8
  %livcase_402 = insertvalue %State %iivcase_399, i64 %ldcase_401, 1
  %iivcase_403 = insertvalue %State %livcase_402, i64 42, 2
  %iivcase_404 = insertvalue %State %iivcase_403, i64 0, 3
  %iivcase_405 = insertvalue %State %iivcase_404, i64 0, 4
  %iivcase_406 = insertvalue %State %iivcase_405, i64 0, 5
  %iivcase_407 = insertvalue %State %iivcase_406, i64 1, 6
  %iivcase_408 = insertvalue %State %iivcase_407, i64 2, 7
  %iivcase_409 = insertvalue %State %iivcase_408, i64 3, 8
  %iivcase_410 = insertvalue %State %iivcase_409, i64 4, 9
  %iivcase_411 = insertvalue %State %iivcase_410, i64 5, 10
  %iivcase_412 = insertvalue %State %iivcase_411, i64 6, 11
  %iivcase_413 = insertvalue %State %iivcase_412, i64 7, 12
  %iivcase_414 = insertvalue %State %iivcase_413, i64 8, 13
  %iivcase_415 = insertvalue %State %iivcase_414, i64 9, 14
  %iivcase_416 = insertvalue %State %iivcase_415, i64 10, 15
  %iivcase_417 = insertvalue %State %iivcase_416, i64 11, 16
  %slot_case = alloca %State, align 8
  store %State %iivcase_417, %State* %slot_case, align 8
  br label %case_hdr
case_hdr:
  %ssa_phi_case = load %State, %State* %slot_case, align 8
  %excase_418 = extractvalue %State %ssa_phi_case, 0
  %adjcase_419 = add i64 %ltcase_4, -3
  %cpcase_420 = icmp slt i64 %excase_418, %adjcase_419
  br i1 %cpcase_420, label %case_body4, label %case_rem
case_rem:
  %cpcase_421 = icmp slt i64 %excase_418, %ltcase_4
  br i1 %cpcase_421, label %case_body1, label %case_done
case_body4:
  %checksum_old_4 = extractvalue %State %ssa_phi_case, 4
  %p4_old_5 = extractvalue %State %ssa_phi_case, 9
  %p10_old_6 = extractvalue %State %ssa_phi_case, 15
  %p11_old_7 = extractvalue %State %ssa_phi_case, 16
  %p1_old_8 = extractvalue %State %ssa_phi_case, 6
  %p6_old_9 = extractvalue %State %ssa_phi_case, 11
  %N_old_10 = extractvalue %State %ssa_phi_case, 1
  %p3_old_11 = extractvalue %State %ssa_phi_case, 8
  %count_old_12 = extractvalue %State %ssa_phi_case, 0
  %max_flips_old_13 = extractvalue %State %ssa_phi_case, 3
  %p7_old_14 = extractvalue %State %ssa_phi_case, 12
  %p8_old_15 = extractvalue %State %ssa_phi_case, 13
  %p9_old_16 = extractvalue %State %ssa_phi_case, 14
  %seed_old_17 = extractvalue %State %ssa_phi_case, 2
  %p0_old_18 = extractvalue %State %ssa_phi_case, 5
  %p5_old_19 = extractvalue %State %ssa_phi_case, 10
  %p2_old_20 = extractvalue %State %ssa_phi_case, 7
  %t22 = add i64 0, %seed_old_17
  %t23 = add i64 0, 3877
  %t21 = mul i64 %t22, %t23
  %in24 = insertvalue %State %ssa_phi_case, i64 %t21, 2
  %t26 = add i64 0, %seed_old_17
  %t27 = add i64 0, 29573
  %t25 = add i64 %t26, %t27
  %in28 = insertvalue %State %in24, i64 %t25, 2
  %t30 = add i64 0, %seed_old_17
  %t31 = add i64 0, 139968
  %t29 = srem i64 %t30, %t31
  %in32 = insertvalue %State %in28, i64 %t29, 2
  %t33 = add i64 0, %p0_old_18
  %in34 = insertvalue %State %in32, i64 %t33, 2
  %t35 = add i64 0, %p1_old_8
  %in36 = insertvalue %State %in34, i64 %t35, 5
  %t37 = add i64 0, %p2_old_20
  %in38 = insertvalue %State %in36, i64 %t37, 6
  %t39 = add i64 0, %p3_old_11
  %in40 = insertvalue %State %in38, i64 %t39, 7
  %t41 = add i64 0, %p4_old_5
  %in42 = insertvalue %State %in40, i64 %t41, 8
  %t43 = add i64 0, %p5_old_19
  %in44 = insertvalue %State %in42, i64 %t43, 9
  %t45 = add i64 0, %p6_old_9
  %in46 = insertvalue %State %in44, i64 %t45, 10
  %t47 = add i64 0, %p7_old_14
  %in48 = insertvalue %State %in46, i64 %t47, 11
  %t49 = add i64 0, %p8_old_15
  %in50 = insertvalue %State %in48, i64 %t49, 12
  %t51 = add i64 0, %p9_old_16
  %in52 = insertvalue %State %in50, i64 %t51, 13
  %t53 = add i64 0, %p10_old_6
  %in54 = insertvalue %State %in52, i64 %t53, 14
  %t55 = add i64 0, %p11_old_7
  %in56 = insertvalue %State %in54, i64 %t55, 15
  %t57 = add i64 0, %seed_old_17
  %in58 = insertvalue %State %in56, i64 %t57, 16
  %t60 = add i64 0, %checksum_old_4
  %t62 = add i64 0, %seed_old_17
  %t63 = add i64 0, 13
  %t61 = srem i64 %t62, %t63
  %t59 = add i64 %t60, %t61
  %in64 = insertvalue %State %in58, i64 %t59, 4
  %t66 = add i64 0, %max_flips_old_13
  %t68 = add i64 0, %checksum_old_4
  %t69 = add i64 0, 17
  %t67 = srem i64 %t68, %t69
  %t65 = add i64 %t66, %t67
  %in70 = insertvalue %State %in64, i64 %t65, 3
  %t72 = add i64 0, %count_old_12
  %t73 = add i64 0, 1
  %t71 = add i64 %t72, %t73
  %in74 = insertvalue %State %in70, i64 %t71, 0
  %t76 = add i64 0, %count_old_12
  %t77 = add i64 0, %N_old_10
  %c78 = icmp eq i64 %t76, %t77
  %t75 = zext i1 %c78 to i64
  %gc79 = icmp ne i64 %t75, 0
  br i1 %gc79, label %g80_t, label %g80_e
  g80_t:
    %t82 = add i64 0, %checksum_old_4
    %t81 = call i64 @__print_int(i64 %t82) #6
    ret i32 0
  g80_e:
  %checksum_old_83 = extractvalue %State %in74, 4
  %p4_old_84 = extractvalue %State %in74, 9
  %p10_old_85 = extractvalue %State %in74, 15
  %p11_old_86 = extractvalue %State %in74, 16
  %p1_old_87 = extractvalue %State %in74, 6
  %p6_old_88 = extractvalue %State %in74, 11
  %N_old_89 = extractvalue %State %in74, 1
  %p3_old_90 = extractvalue %State %in74, 8
  %count_old_91 = extractvalue %State %in74, 0
  %max_flips_old_92 = extractvalue %State %in74, 3
  %p7_old_93 = extractvalue %State %in74, 12
  %p8_old_94 = extractvalue %State %in74, 13
  %p9_old_95 = extractvalue %State %in74, 14
  %seed_old_96 = extractvalue %State %in74, 2
  %p0_old_97 = extractvalue %State %in74, 5
  %p5_old_98 = extractvalue %State %in74, 10
  %p2_old_99 = extractvalue %State %in74, 7
  %t101 = add i64 0, %seed_old_96
  %t102 = add i64 0, 3877
  %t100 = mul i64 %t101, %t102
  %in103 = insertvalue %State %in74, i64 %t100, 2
  %t105 = add i64 0, %seed_old_96
  %t106 = add i64 0, 29573
  %t104 = add i64 %t105, %t106
  %in107 = insertvalue %State %in103, i64 %t104, 2
  %t109 = add i64 0, %seed_old_96
  %t110 = add i64 0, 139968
  %t108 = srem i64 %t109, %t110
  %in111 = insertvalue %State %in107, i64 %t108, 2
  %t112 = add i64 0, %p0_old_97
  %in113 = insertvalue %State %in111, i64 %t112, 2
  %t114 = add i64 0, %p1_old_87
  %in115 = insertvalue %State %in113, i64 %t114, 5
  %t116 = add i64 0, %p2_old_99
  %in117 = insertvalue %State %in115, i64 %t116, 6
  %t118 = add i64 0, %p3_old_90
  %in119 = insertvalue %State %in117, i64 %t118, 7
  %t120 = add i64 0, %p4_old_84
  %in121 = insertvalue %State %in119, i64 %t120, 8
  %t122 = add i64 0, %p5_old_98
  %in123 = insertvalue %State %in121, i64 %t122, 9
  %t124 = add i64 0, %p6_old_88
  %in125 = insertvalue %State %in123, i64 %t124, 10
  %t126 = add i64 0, %p7_old_93
  %in127 = insertvalue %State %in125, i64 %t126, 11
  %t128 = add i64 0, %p8_old_94
  %in129 = insertvalue %State %in127, i64 %t128, 12
  %t130 = add i64 0, %p9_old_95
  %in131 = insertvalue %State %in129, i64 %t130, 13
  %t132 = add i64 0, %p10_old_85
  %in133 = insertvalue %State %in131, i64 %t132, 14
  %t134 = add i64 0, %p11_old_86
  %in135 = insertvalue %State %in133, i64 %t134, 15
  %t136 = add i64 0, %seed_old_96
  %in137 = insertvalue %State %in135, i64 %t136, 16
  %t139 = add i64 0, %checksum_old_83
  %t141 = add i64 0, %seed_old_96
  %t142 = add i64 0, 13
  %t140 = srem i64 %t141, %t142
  %t138 = add i64 %t139, %t140
  %in143 = insertvalue %State %in137, i64 %t138, 4
  %t145 = add i64 0, %max_flips_old_92
  %t147 = add i64 0, %checksum_old_83
  %t148 = add i64 0, 17
  %t146 = srem i64 %t147, %t148
  %t144 = add i64 %t145, %t146
  %in149 = insertvalue %State %in143, i64 %t144, 3
  %t151 = add i64 0, %count_old_91
  %t152 = add i64 0, 1
  %t150 = add i64 %t151, %t152
  %in153 = insertvalue %State %in149, i64 %t150, 0
  %t155 = add i64 0, %count_old_91
  %t156 = add i64 0, %N_old_89
  %c157 = icmp eq i64 %t155, %t156
  %t154 = zext i1 %c157 to i64
  %gc158 = icmp ne i64 %t154, 0
  br i1 %gc158, label %g159_t, label %g159_e
  g159_t:
    %t161 = add i64 0, %checksum_old_83
    %t160 = call i64 @__print_int(i64 %t161) #6
    ret i32 0
  g159_e:
  %checksum_old_162 = extractvalue %State %in153, 4
  %p4_old_163 = extractvalue %State %in153, 9
  %p10_old_164 = extractvalue %State %in153, 15
  %p11_old_165 = extractvalue %State %in153, 16
  %p1_old_166 = extractvalue %State %in153, 6
  %p6_old_167 = extractvalue %State %in153, 11
  %N_old_168 = extractvalue %State %in153, 1
  %p3_old_169 = extractvalue %State %in153, 8
  %count_old_170 = extractvalue %State %in153, 0
  %max_flips_old_171 = extractvalue %State %in153, 3
  %p7_old_172 = extractvalue %State %in153, 12
  %p8_old_173 = extractvalue %State %in153, 13
  %p9_old_174 = extractvalue %State %in153, 14
  %seed_old_175 = extractvalue %State %in153, 2
  %p0_old_176 = extractvalue %State %in153, 5
  %p5_old_177 = extractvalue %State %in153, 10
  %p2_old_178 = extractvalue %State %in153, 7
  %t180 = add i64 0, %seed_old_175
  %t181 = add i64 0, 3877
  %t179 = mul i64 %t180, %t181
  %in182 = insertvalue %State %in153, i64 %t179, 2
  %t184 = add i64 0, %seed_old_175
  %t185 = add i64 0, 29573
  %t183 = add i64 %t184, %t185
  %in186 = insertvalue %State %in182, i64 %t183, 2
  %t188 = add i64 0, %seed_old_175
  %t189 = add i64 0, 139968
  %t187 = srem i64 %t188, %t189
  %in190 = insertvalue %State %in186, i64 %t187, 2
  %t191 = add i64 0, %p0_old_176
  %in192 = insertvalue %State %in190, i64 %t191, 2
  %t193 = add i64 0, %p1_old_166
  %in194 = insertvalue %State %in192, i64 %t193, 5
  %t195 = add i64 0, %p2_old_178
  %in196 = insertvalue %State %in194, i64 %t195, 6
  %t197 = add i64 0, %p3_old_169
  %in198 = insertvalue %State %in196, i64 %t197, 7
  %t199 = add i64 0, %p4_old_163
  %in200 = insertvalue %State %in198, i64 %t199, 8
  %t201 = add i64 0, %p5_old_177
  %in202 = insertvalue %State %in200, i64 %t201, 9
  %t203 = add i64 0, %p6_old_167
  %in204 = insertvalue %State %in202, i64 %t203, 10
  %t205 = add i64 0, %p7_old_172
  %in206 = insertvalue %State %in204, i64 %t205, 11
  %t207 = add i64 0, %p8_old_173
  %in208 = insertvalue %State %in206, i64 %t207, 12
  %t209 = add i64 0, %p9_old_174
  %in210 = insertvalue %State %in208, i64 %t209, 13
  %t211 = add i64 0, %p10_old_164
  %in212 = insertvalue %State %in210, i64 %t211, 14
  %t213 = add i64 0, %p11_old_165
  %in214 = insertvalue %State %in212, i64 %t213, 15
  %t215 = add i64 0, %seed_old_175
  %in216 = insertvalue %State %in214, i64 %t215, 16
  %t218 = add i64 0, %checksum_old_162
  %t220 = add i64 0, %seed_old_175
  %t221 = add i64 0, 13
  %t219 = srem i64 %t220, %t221
  %t217 = add i64 %t218, %t219
  %in222 = insertvalue %State %in216, i64 %t217, 4
  %t224 = add i64 0, %max_flips_old_171
  %t226 = add i64 0, %checksum_old_162
  %t227 = add i64 0, 17
  %t225 = srem i64 %t226, %t227
  %t223 = add i64 %t224, %t225
  %in228 = insertvalue %State %in222, i64 %t223, 3
  %t230 = add i64 0, %count_old_170
  %t231 = add i64 0, 1
  %t229 = add i64 %t230, %t231
  %in232 = insertvalue %State %in228, i64 %t229, 0
  %t234 = add i64 0, %count_old_170
  %t235 = add i64 0, %N_old_168
  %c236 = icmp eq i64 %t234, %t235
  %t233 = zext i1 %c236 to i64
  %gc237 = icmp ne i64 %t233, 0
  br i1 %gc237, label %g238_t, label %g238_e
  g238_t:
    %t240 = add i64 0, %checksum_old_162
    %t239 = call i64 @__print_int(i64 %t240) #6
    ret i32 0
  g238_e:
  %checksum_old_241 = extractvalue %State %in232, 4
  %p4_old_242 = extractvalue %State %in232, 9
  %p10_old_243 = extractvalue %State %in232, 15
  %p11_old_244 = extractvalue %State %in232, 16
  %p1_old_245 = extractvalue %State %in232, 6
  %p6_old_246 = extractvalue %State %in232, 11
  %N_old_247 = extractvalue %State %in232, 1
  %p3_old_248 = extractvalue %State %in232, 8
  %count_old_249 = extractvalue %State %in232, 0
  %max_flips_old_250 = extractvalue %State %in232, 3
  %p7_old_251 = extractvalue %State %in232, 12
  %p8_old_252 = extractvalue %State %in232, 13
  %p9_old_253 = extractvalue %State %in232, 14
  %seed_old_254 = extractvalue %State %in232, 2
  %p0_old_255 = extractvalue %State %in232, 5
  %p5_old_256 = extractvalue %State %in232, 10
  %p2_old_257 = extractvalue %State %in232, 7
  %t259 = add i64 0, %seed_old_254
  %t260 = add i64 0, 3877
  %t258 = mul i64 %t259, %t260
  %in261 = insertvalue %State %in232, i64 %t258, 2
  %t263 = add i64 0, %seed_old_254
  %t264 = add i64 0, 29573
  %t262 = add i64 %t263, %t264
  %in265 = insertvalue %State %in261, i64 %t262, 2
  %t267 = add i64 0, %seed_old_254
  %t268 = add i64 0, 139968
  %t266 = srem i64 %t267, %t268
  %in269 = insertvalue %State %in265, i64 %t266, 2
  %t270 = add i64 0, %p0_old_255
  %in271 = insertvalue %State %in269, i64 %t270, 2
  %t272 = add i64 0, %p1_old_245
  %in273 = insertvalue %State %in271, i64 %t272, 5
  %t274 = add i64 0, %p2_old_257
  %in275 = insertvalue %State %in273, i64 %t274, 6
  %t276 = add i64 0, %p3_old_248
  %in277 = insertvalue %State %in275, i64 %t276, 7
  %t278 = add i64 0, %p4_old_242
  %in279 = insertvalue %State %in277, i64 %t278, 8
  %t280 = add i64 0, %p5_old_256
  %in281 = insertvalue %State %in279, i64 %t280, 9
  %t282 = add i64 0, %p6_old_246
  %in283 = insertvalue %State %in281, i64 %t282, 10
  %t284 = add i64 0, %p7_old_251
  %in285 = insertvalue %State %in283, i64 %t284, 11
  %t286 = add i64 0, %p8_old_252
  %in287 = insertvalue %State %in285, i64 %t286, 12
  %t288 = add i64 0, %p9_old_253
  %in289 = insertvalue %State %in287, i64 %t288, 13
  %t290 = add i64 0, %p10_old_243
  %in291 = insertvalue %State %in289, i64 %t290, 14
  %t292 = add i64 0, %p11_old_244
  %in293 = insertvalue %State %in291, i64 %t292, 15
  %t294 = add i64 0, %seed_old_254
  %in295 = insertvalue %State %in293, i64 %t294, 16
  %t297 = add i64 0, %checksum_old_241
  %t299 = add i64 0, %seed_old_254
  %t300 = add i64 0, 13
  %t298 = srem i64 %t299, %t300
  %t296 = add i64 %t297, %t298
  %in301 = insertvalue %State %in295, i64 %t296, 4
  %t303 = add i64 0, %max_flips_old_250
  %t305 = add i64 0, %checksum_old_241
  %t306 = add i64 0, 17
  %t304 = srem i64 %t305, %t306
  %t302 = add i64 %t303, %t304
  %in307 = insertvalue %State %in301, i64 %t302, 3
  %t309 = add i64 0, %count_old_249
  %t310 = add i64 0, 1
  %t308 = add i64 %t309, %t310
  %in311 = insertvalue %State %in307, i64 %t308, 0
  %t313 = add i64 0, %count_old_249
  %t314 = add i64 0, %N_old_247
  %c315 = icmp eq i64 %t313, %t314
  %t312 = zext i1 %c315 to i64
  %gc316 = icmp ne i64 %t312, 0
  br i1 %gc316, label %g317_t, label %g317_e
  g317_t:
    %t319 = add i64 0, %checksum_old_241
    %t318 = call i64 @__print_int(i64 %t319) #6
    ret i32 0
  g317_e:
  store %State %in311, %State* %slot_case, align 8
  br label %case_hdr
case_body1:
  %checksum_old_320 = extractvalue %State %ssa_phi_case, 4
  %p4_old_321 = extractvalue %State %ssa_phi_case, 9
  %p10_old_322 = extractvalue %State %ssa_phi_case, 15
  %p11_old_323 = extractvalue %State %ssa_phi_case, 16
  %p1_old_324 = extractvalue %State %ssa_phi_case, 6
  %p6_old_325 = extractvalue %State %ssa_phi_case, 11
  %N_old_326 = extractvalue %State %ssa_phi_case, 1
  %p3_old_327 = extractvalue %State %ssa_phi_case, 8
  %count_old_328 = extractvalue %State %ssa_phi_case, 0
  %max_flips_old_329 = extractvalue %State %ssa_phi_case, 3
  %p7_old_330 = extractvalue %State %ssa_phi_case, 12
  %p8_old_331 = extractvalue %State %ssa_phi_case, 13
  %p9_old_332 = extractvalue %State %ssa_phi_case, 14
  %seed_old_333 = extractvalue %State %ssa_phi_case, 2
  %p0_old_334 = extractvalue %State %ssa_phi_case, 5
  %p5_old_335 = extractvalue %State %ssa_phi_case, 10
  %p2_old_336 = extractvalue %State %ssa_phi_case, 7
  %t338 = add i64 0, %seed_old_333
  %t339 = add i64 0, 3877
  %t337 = mul i64 %t338, %t339
  %in340 = insertvalue %State %ssa_phi_case, i64 %t337, 2
  %t342 = add i64 0, %seed_old_333
  %t343 = add i64 0, 29573
  %t341 = add i64 %t342, %t343
  %in344 = insertvalue %State %in340, i64 %t341, 2
  %t346 = add i64 0, %seed_old_333
  %t347 = add i64 0, 139968
  %t345 = srem i64 %t346, %t347
  %in348 = insertvalue %State %in344, i64 %t345, 2
  %t349 = add i64 0, %p0_old_334
  %in350 = insertvalue %State %in348, i64 %t349, 2
  %t351 = add i64 0, %p1_old_324
  %in352 = insertvalue %State %in350, i64 %t351, 5
  %t353 = add i64 0, %p2_old_336
  %in354 = insertvalue %State %in352, i64 %t353, 6
  %t355 = add i64 0, %p3_old_327
  %in356 = insertvalue %State %in354, i64 %t355, 7
  %t357 = add i64 0, %p4_old_321
  %in358 = insertvalue %State %in356, i64 %t357, 8
  %t359 = add i64 0, %p5_old_335
  %in360 = insertvalue %State %in358, i64 %t359, 9
  %t361 = add i64 0, %p6_old_325
  %in362 = insertvalue %State %in360, i64 %t361, 10
  %t363 = add i64 0, %p7_old_330
  %in364 = insertvalue %State %in362, i64 %t363, 11
  %t365 = add i64 0, %p8_old_331
  %in366 = insertvalue %State %in364, i64 %t365, 12
  %t367 = add i64 0, %p9_old_332
  %in368 = insertvalue %State %in366, i64 %t367, 13
  %t369 = add i64 0, %p10_old_322
  %in370 = insertvalue %State %in368, i64 %t369, 14
  %t371 = add i64 0, %p11_old_323
  %in372 = insertvalue %State %in370, i64 %t371, 15
  %t373 = add i64 0, %seed_old_333
  %in374 = insertvalue %State %in372, i64 %t373, 16
  %t376 = add i64 0, %checksum_old_320
  %t378 = add i64 0, %seed_old_333
  %t379 = add i64 0, 13
  %t377 = srem i64 %t378, %t379
  %t375 = add i64 %t376, %t377
  %in380 = insertvalue %State %in374, i64 %t375, 4
  %t382 = add i64 0, %max_flips_old_329
  %t384 = add i64 0, %checksum_old_320
  %t385 = add i64 0, 17
  %t383 = srem i64 %t384, %t385
  %t381 = add i64 %t382, %t383
  %in386 = insertvalue %State %in380, i64 %t381, 3
  %t388 = add i64 0, %count_old_328
  %t389 = add i64 0, 1
  %t387 = add i64 %t388, %t389
  %in390 = insertvalue %State %in386, i64 %t387, 0
  %t392 = add i64 0, %count_old_328
  %t393 = add i64 0, %N_old_326
  %c394 = icmp eq i64 %t392, %t393
  %t391 = zext i1 %c394 to i64
  %gc395 = icmp ne i64 %t391, 0
  br i1 %gc395, label %g396_t, label %g396_e
  g396_t:
    %t398 = add i64 0, %checksum_old_320
    %t397 = call i64 @__print_int(i64 %t398) #6
    ret i32 0
  g396_e:
  store %State %in390, %State* %slot_case, align 8
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
