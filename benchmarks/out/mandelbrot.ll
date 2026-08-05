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
@IA = constant i64 3877
@IC = constant i64 29573
@IM = constant i64 139968
@SCALE = constant i64 100
@THRESH = constant i64 40000

%State = type { i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant [6 x i8] c"BOUND\00", align 1

define void @mb(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
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
  %fdp32 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il33 = load i64, i64* %fdp32, align 8
  %t31 = add i64 0, %il33
  %t34 = add i64 0, 200
  %t30 = srem i64 %t31, %t34
  %t35 = add i64 0, 100
  %t29 = sub i64 %t30, %t35
  %ap36 = getelementptr inbounds %State, %State* %state, i32 0, i32 5
  store i64 %t29, i64* %ap36, align 8
  %fdp39 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il40 = load i64, i64* %fdp39, align 8
  %t38 = add i64 0, %il40
  %t41 = add i64 0, 3877
  %t37 = mul i64 %t38, %t41
  %ap42 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  store i64 %t37, i64* %ap42, align 8
  %fdp45 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il46 = load i64, i64* %fdp45, align 8
  %t44 = add i64 0, %il46
  %t47 = add i64 0, 29573
  %t43 = add i64 %t44, %t47
  %ap48 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  store i64 %t43, i64* %ap48, align 8
  %fdp51 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il52 = load i64, i64* %fdp51, align 8
  %t50 = add i64 0, %il52
  %t53 = add i64 0, 139968
  %t49 = srem i64 %t50, %t53
  %ap54 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  store i64 %t49, i64* %ap54, align 8
  %fdp58 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il59 = load i64, i64* %fdp58, align 8
  %t57 = add i64 0, %il59
  %t60 = add i64 0, 200
  %t56 = srem i64 %t57, %t60
  %t61 = add i64 0, 100
  %t55 = sub i64 %t56, %t61
  %ap62 = getelementptr inbounds %State, %State* %state, i32 0, i32 6
  store i64 %t55, i64* %ap62, align 8
  %fdp66 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  %il67 = load i64, i64* %fdp66, align 8
  %t65 = add i64 0, %il67
  %fdp69 = getelementptr inbounds %State, %State* %state, i32 0, i32 5
  %il70 = load i64, i64* %fdp69, align 8
  %t68 = add i64 0, %il70
  %t64 = mul i64 %t65, %t68
  %t71 = add i64 0, 100
  %t63 = sdiv i64 %t64, %t71
  %ap72 = getelementptr inbounds %State, %State* %state, i32 0, i32 7
  store i64 %t63, i64* %ap72, align 8
  %fdp76 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  %il77 = load i64, i64* %fdp76, align 8
  %t75 = add i64 0, %il77
  %fdp79 = getelementptr inbounds %State, %State* %state, i32 0, i32 6
  %il80 = load i64, i64* %fdp79, align 8
  %t78 = add i64 0, %il80
  %t74 = mul i64 %t75, %t78
  %t81 = add i64 0, 100
  %t73 = sdiv i64 %t74, %t81
  %ap82 = getelementptr inbounds %State, %State* %state, i32 0, i32 8
  store i64 %t73, i64* %ap82, align 8
  %fdp86 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  %il87 = load i64, i64* %fdp86, align 8
  %t85 = add i64 0, %il87
  %fdp89 = getelementptr inbounds %State, %State* %state, i32 0, i32 6
  %il90 = load i64, i64* %fdp89, align 8
  %t88 = add i64 0, %il90
  %t84 = mul i64 %t85, %t88
  %t91 = add i64 0, 100
  %t83 = sdiv i64 %t84, %t91
  %ap92 = getelementptr inbounds %State, %State* %state, i32 0, i32 9
  store i64 %t83, i64* %ap92, align 8
  %fdp95 = getelementptr inbounds %State, %State* %state, i32 0, i32 7
  %il96 = load i64, i64* %fdp95, align 8
  %t94 = add i64 0, %il96
  %fdp98 = getelementptr inbounds %State, %State* %state, i32 0, i32 8
  %il99 = load i64, i64* %fdp98, align 8
  %t97 = add i64 0, %il99
  %t93 = sub i64 %t94, %t97
  %ap100 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  store i64 %t93, i64* %ap100, align 8
  %fdp103 = getelementptr inbounds %State, %State* %state, i32 0, i32 9
  %il104 = load i64, i64* %fdp103, align 8
  %t102 = add i64 0, %il104
  %fdp108 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  %il109 = load i64, i64* %fdp108, align 8
  %t107 = add i64 0, %il109
  %fdp111 = getelementptr inbounds %State, %State* %state, i32 0, i32 5
  %il112 = load i64, i64* %fdp111, align 8
  %t110 = add i64 0, %il112
  %t106 = mul i64 %t107, %t110
  %t113 = add i64 0, 100
  %t105 = sdiv i64 %t106, %t113
  %t101 = add i64 %t102, %t105
  %ap114 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  store i64 %t101, i64* %ap114, align 8
  %fdp118 = getelementptr inbounds %State, %State* %state, i32 0, i32 10
  %il119 = load i64, i64* %fdp118, align 8
  %t117 = add i64 0, %il119
  %fdp123 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  %il124 = load i64, i64* %fdp123, align 8
  %t122 = add i64 0, %il124
  %fdp126 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  %il127 = load i64, i64* %fdp126, align 8
  %t125 = add i64 0, %il127
  %t121 = mul i64 %t122, %t125
  %t128 = add i64 0, 100
  %t120 = sdiv i64 %t121, %t128
  %t116 = add i64 %t117, %t120
  %fdp132 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  %il133 = load i64, i64* %fdp132, align 8
  %t131 = add i64 0, %il133
  %fdp135 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  %il136 = load i64, i64* %fdp135, align 8
  %t134 = add i64 0, %il136
  %t130 = mul i64 %t131, %t134
  %t137 = add i64 0, 100
  %t129 = sdiv i64 %t130, %t137
  %t115 = add i64 %t116, %t129
  %ap138 = getelementptr inbounds %State, %State* %state, i32 0, i32 10
  store i64 %t115, i64* %ap138, align 8
  %fdp142 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il143 = load i64, i64* %fdp142, align 8
  %t141 = add i64 0, %il143
  %t144 = add i64 0, 5000000
  %t140 = srem i64 %t141, %t144
  %t145 = add i64 0, 0
  %c146 = icmp eq i64 %t140, %t145
  %t139 = zext i1 %c146 to i64
  %gc147 = icmp ne i64 %t139, 0
  br i1 %gc147, label %g148_t, label %g148_e
  g148_t:
    %fdp151 = getelementptr inbounds %State, %State* %state, i32 0, i32 10
    %il152 = load i64, i64* %fdp151, align 8
    %t150 = add i64 0, %il152
    %t149 = call i64 @__print_int(i64 %t150) #6
    br label %g148_e
  g148_e:
  %fdp155 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il156 = load i64, i64* %fdp155, align 8
  %t154 = add i64 0, %il156
  %t157 = add i64 0, 1
  %t153 = add i64 %t154, %t157
  %ap158 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store i64 %t153, i64* %ap158, align 8
  %fdp161 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il162 = load i64, i64* %fdp161, align 8
  %t160 = add i64 0, %il162
  %fdp164 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il165 = load i64, i64* %fdp164, align 8
  %t163 = add i64 0, %il165
  %c166 = icmp eq i64 %t160, %t163
  %t159 = zext i1 %c166 to i64
  %gc167 = icmp ne i64 %t159, 0
  br i1 %gc167, label %g168_t, label %g168_e
  g168_t:
    %fdp171 = getelementptr inbounds %State, %State* %state, i32 0, i32 10
    %il172 = load i64, i64* %fdp171, align 8
    %t170 = add i64 0, %il172
    %t169 = call i64 @__print_int(i64 %t170) #6
    ret void
  g168_e:
  ret void
}

define internal i1 @pre_mb(%State* noalias nocapture %state) #0 {
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
  store i64 100, i64* %ip3, align 8
  %ip4 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  store i64 0, i64* %ip4, align 8
  %ip5 = getelementptr inbounds %State, %State* %state, i32 0, i32 5
  store i64 -75, i64* %ip5, align 8
  %ip6 = getelementptr inbounds %State, %State* %state, i32 0, i32 6
  store i64 10, i64* %ip6, align 8
  %ip7 = getelementptr inbounds %State, %State* %state, i32 0, i32 7
  store i64 0, i64* %ip7, align 8
  %ip8 = getelementptr inbounds %State, %State* %state, i32 0, i32 8
  store i64 0, i64* %ip8, align 8
  %ip9 = getelementptr inbounds %State, %State* %state, i32 0, i32 9
  store i64 0, i64* %ip9, align 8
  %ip10 = getelementptr inbounds %State, %State* %state, i32 0, i32 10
  store i64 0, i64* %ip10, align 8
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
  %iivcase_569 = insertvalue %State zeroinitializer, i64 0, 0
  %gepcase_570 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %ldcase_571 = load i64, i64* %gepcase_570, align 8
  %livcase_572 = insertvalue %State %iivcase_569, i64 %ldcase_571, 1
  %iivcase_573 = insertvalue %State %livcase_572, i64 42, 2
  %iivcase_574 = insertvalue %State %iivcase_573, i64 100, 3
  %iivcase_575 = insertvalue %State %iivcase_574, i64 0, 4
  %nivcase_576 = insertvalue %State %iivcase_575, i64 -75, 5
  %iivcase_577 = insertvalue %State %nivcase_576, i64 10, 6
  %iivcase_578 = insertvalue %State %iivcase_577, i64 0, 7
  %iivcase_579 = insertvalue %State %iivcase_578, i64 0, 8
  %iivcase_580 = insertvalue %State %iivcase_579, i64 0, 9
  %iivcase_581 = insertvalue %State %iivcase_580, i64 0, 10
  %slot_case = alloca %State, align 8
  store %State %iivcase_581, %State* %slot_case, align 8
  br label %case_hdr
case_hdr:
  %ssa_phi_case = load %State, %State* %slot_case, align 8
  %excase_582 = extractvalue %State %ssa_phi_case, 0
  %adjcase_583 = add i64 %ltcase_4, -3
  %cpcase_584 = icmp slt i64 %excase_582, %adjcase_583
  br i1 %cpcase_584, label %case_body4, label %case_rem
case_rem:
  %cpcase_585 = icmp slt i64 %excase_582, %ltcase_4
  br i1 %cpcase_585, label %case_body1, label %case_done
case_body4:
  %seed_old_4 = extractvalue %State %ssa_phi_case, 2
  %escapes_old_5 = extractvalue %State %ssa_phi_case, 10
  %t1_old_6 = extractvalue %State %ssa_phi_case, 7
  %N_old_7 = extractvalue %State %ssa_phi_case, 1
  %zi_old_8 = extractvalue %State %ssa_phi_case, 4
  %ci_old_9 = extractvalue %State %ssa_phi_case, 6
  %t3_old_10 = extractvalue %State %ssa_phi_case, 9
  %cr_old_11 = extractvalue %State %ssa_phi_case, 5
  %t2_old_12 = extractvalue %State %ssa_phi_case, 8
  %count_old_13 = extractvalue %State %ssa_phi_case, 0
  %zr_old_14 = extractvalue %State %ssa_phi_case, 3
  %t16 = add i64 0, %seed_old_4
  %t17 = add i64 0, 3877
  %t15 = mul i64 %t16, %t17
  %in18 = insertvalue %State %ssa_phi_case, i64 %t15, 2
  %t20 = add i64 0, %seed_old_4
  %t21 = add i64 0, 29573
  %t19 = add i64 %t20, %t21
  %in22 = insertvalue %State %in18, i64 %t19, 2
  %t24 = add i64 0, %seed_old_4
  %t25 = add i64 0, 139968
  %t23 = srem i64 %t24, %t25
  %in26 = insertvalue %State %in22, i64 %t23, 2
  %t29 = add i64 0, %seed_old_4
  %t30 = add i64 0, 200
  %t28 = srem i64 %t29, %t30
  %t31 = add i64 0, 100
  %t27 = sub i64 %t28, %t31
  %in32 = insertvalue %State %in26, i64 %t27, 5
  %t34 = add i64 0, %seed_old_4
  %t35 = add i64 0, 3877
  %t33 = mul i64 %t34, %t35
  %in36 = insertvalue %State %in32, i64 %t33, 2
  %t38 = add i64 0, %seed_old_4
  %t39 = add i64 0, 29573
  %t37 = add i64 %t38, %t39
  %in40 = insertvalue %State %in36, i64 %t37, 2
  %t42 = add i64 0, %seed_old_4
  %t43 = add i64 0, 139968
  %t41 = srem i64 %t42, %t43
  %in44 = insertvalue %State %in40, i64 %t41, 2
  %t47 = add i64 0, %seed_old_4
  %t48 = add i64 0, 200
  %t46 = srem i64 %t47, %t48
  %t49 = add i64 0, 100
  %t45 = sub i64 %t46, %t49
  %in50 = insertvalue %State %in44, i64 %t45, 6
  %t53 = add i64 0, %zr_old_14
  %t54 = add i64 0, %cr_old_11
  %t52 = mul i64 %t53, %t54
  %t55 = add i64 0, 100
  %t51 = sdiv i64 %t52, %t55
  %in56 = insertvalue %State %in50, i64 %t51, 7
  %t59 = add i64 0, %zi_old_8
  %t60 = add i64 0, %ci_old_9
  %t58 = mul i64 %t59, %t60
  %t61 = add i64 0, 100
  %t57 = sdiv i64 %t58, %t61
  %in62 = insertvalue %State %in56, i64 %t57, 8
  %t65 = add i64 0, %zr_old_14
  %t66 = add i64 0, %ci_old_9
  %t64 = mul i64 %t65, %t66
  %t67 = add i64 0, 100
  %t63 = sdiv i64 %t64, %t67
  %in68 = insertvalue %State %in62, i64 %t63, 9
  %t70 = add i64 0, %t1_old_6
  %t71 = add i64 0, %t2_old_12
  %t69 = sub i64 %t70, %t71
  %in72 = insertvalue %State %in68, i64 %t69, 3
  %t74 = add i64 0, %t3_old_10
  %t77 = add i64 0, %zi_old_8
  %t78 = add i64 0, %cr_old_11
  %t76 = mul i64 %t77, %t78
  %t79 = add i64 0, 100
  %t75 = sdiv i64 %t76, %t79
  %t73 = add i64 %t74, %t75
  %in80 = insertvalue %State %in72, i64 %t73, 4
  %t83 = add i64 0, %escapes_old_5
  %t86 = add i64 0, %zr_old_14
  %t87 = add i64 0, %zr_old_14
  %t85 = mul i64 %t86, %t87
  %t88 = add i64 0, 100
  %t84 = sdiv i64 %t85, %t88
  %t82 = add i64 %t83, %t84
  %t91 = add i64 0, %zi_old_8
  %t92 = add i64 0, %zi_old_8
  %t90 = mul i64 %t91, %t92
  %t93 = add i64 0, 100
  %t89 = sdiv i64 %t90, %t93
  %t81 = add i64 %t82, %t89
  %in94 = insertvalue %State %in80, i64 %t81, 10
  %t97 = add i64 0, %count_old_13
  %t98 = add i64 0, 5000000
  %t96 = srem i64 %t97, %t98
  %t99 = add i64 0, 0
  %c100 = icmp eq i64 %t96, %t99
  %t95 = zext i1 %c100 to i64
  %gc101 = icmp ne i64 %t95, 0
  br i1 %gc101, label %g102_t, label %g102_e
  g102_t:
    %t104 = add i64 0, %escapes_old_5
    %t103 = call i64 @__print_int(i64 %t104) #6
    br label %g102_e
  g102_e:
  %t106 = add i64 0, %count_old_13
  %t107 = add i64 0, 1
  %t105 = add i64 %t106, %t107
  %in108 = insertvalue %State %in94, i64 %t105, 0
  %t110 = add i64 0, %count_old_13
  %t111 = add i64 0, %N_old_7
  %c112 = icmp eq i64 %t110, %t111
  %t109 = zext i1 %c112 to i64
  %gc113 = icmp ne i64 %t109, 0
  br i1 %gc113, label %g114_t, label %g114_e
  g114_t:
    %t116 = add i64 0, %escapes_old_5
    %t115 = call i64 @__print_int(i64 %t116) #6
    ret i32 0
  g114_e:
  %seed_old_117 = extractvalue %State %in108, 2
  %escapes_old_118 = extractvalue %State %in108, 10
  %t1_old_119 = extractvalue %State %in108, 7
  %N_old_120 = extractvalue %State %in108, 1
  %zi_old_121 = extractvalue %State %in108, 4
  %ci_old_122 = extractvalue %State %in108, 6
  %t3_old_123 = extractvalue %State %in108, 9
  %cr_old_124 = extractvalue %State %in108, 5
  %t2_old_125 = extractvalue %State %in108, 8
  %count_old_126 = extractvalue %State %in108, 0
  %zr_old_127 = extractvalue %State %in108, 3
  %t129 = add i64 0, %seed_old_117
  %t130 = add i64 0, 3877
  %t128 = mul i64 %t129, %t130
  %in131 = insertvalue %State %in108, i64 %t128, 2
  %t133 = add i64 0, %seed_old_117
  %t134 = add i64 0, 29573
  %t132 = add i64 %t133, %t134
  %in135 = insertvalue %State %in131, i64 %t132, 2
  %t137 = add i64 0, %seed_old_117
  %t138 = add i64 0, 139968
  %t136 = srem i64 %t137, %t138
  %in139 = insertvalue %State %in135, i64 %t136, 2
  %t142 = add i64 0, %seed_old_117
  %t143 = add i64 0, 200
  %t141 = srem i64 %t142, %t143
  %t144 = add i64 0, 100
  %t140 = sub i64 %t141, %t144
  %in145 = insertvalue %State %in139, i64 %t140, 5
  %t147 = add i64 0, %seed_old_117
  %t148 = add i64 0, 3877
  %t146 = mul i64 %t147, %t148
  %in149 = insertvalue %State %in145, i64 %t146, 2
  %t151 = add i64 0, %seed_old_117
  %t152 = add i64 0, 29573
  %t150 = add i64 %t151, %t152
  %in153 = insertvalue %State %in149, i64 %t150, 2
  %t155 = add i64 0, %seed_old_117
  %t156 = add i64 0, 139968
  %t154 = srem i64 %t155, %t156
  %in157 = insertvalue %State %in153, i64 %t154, 2
  %t160 = add i64 0, %seed_old_117
  %t161 = add i64 0, 200
  %t159 = srem i64 %t160, %t161
  %t162 = add i64 0, 100
  %t158 = sub i64 %t159, %t162
  %in163 = insertvalue %State %in157, i64 %t158, 6
  %t166 = add i64 0, %zr_old_127
  %t167 = add i64 0, %cr_old_124
  %t165 = mul i64 %t166, %t167
  %t168 = add i64 0, 100
  %t164 = sdiv i64 %t165, %t168
  %in169 = insertvalue %State %in163, i64 %t164, 7
  %t172 = add i64 0, %zi_old_121
  %t173 = add i64 0, %ci_old_122
  %t171 = mul i64 %t172, %t173
  %t174 = add i64 0, 100
  %t170 = sdiv i64 %t171, %t174
  %in175 = insertvalue %State %in169, i64 %t170, 8
  %t178 = add i64 0, %zr_old_127
  %t179 = add i64 0, %ci_old_122
  %t177 = mul i64 %t178, %t179
  %t180 = add i64 0, 100
  %t176 = sdiv i64 %t177, %t180
  %in181 = insertvalue %State %in175, i64 %t176, 9
  %t183 = add i64 0, %t1_old_119
  %t184 = add i64 0, %t2_old_125
  %t182 = sub i64 %t183, %t184
  %in185 = insertvalue %State %in181, i64 %t182, 3
  %t187 = add i64 0, %t3_old_123
  %t190 = add i64 0, %zi_old_121
  %t191 = add i64 0, %cr_old_124
  %t189 = mul i64 %t190, %t191
  %t192 = add i64 0, 100
  %t188 = sdiv i64 %t189, %t192
  %t186 = add i64 %t187, %t188
  %in193 = insertvalue %State %in185, i64 %t186, 4
  %t196 = add i64 0, %escapes_old_118
  %t199 = add i64 0, %zr_old_127
  %t200 = add i64 0, %zr_old_127
  %t198 = mul i64 %t199, %t200
  %t201 = add i64 0, 100
  %t197 = sdiv i64 %t198, %t201
  %t195 = add i64 %t196, %t197
  %t204 = add i64 0, %zi_old_121
  %t205 = add i64 0, %zi_old_121
  %t203 = mul i64 %t204, %t205
  %t206 = add i64 0, 100
  %t202 = sdiv i64 %t203, %t206
  %t194 = add i64 %t195, %t202
  %in207 = insertvalue %State %in193, i64 %t194, 10
  %t210 = add i64 0, %count_old_126
  %t211 = add i64 0, 5000000
  %t209 = srem i64 %t210, %t211
  %t212 = add i64 0, 0
  %c213 = icmp eq i64 %t209, %t212
  %t208 = zext i1 %c213 to i64
  %gc214 = icmp ne i64 %t208, 0
  br i1 %gc214, label %g215_t, label %g215_e
  g215_t:
    %t217 = add i64 0, %escapes_old_118
    %t216 = call i64 @__print_int(i64 %t217) #6
    br label %g215_e
  g215_e:
  %t219 = add i64 0, %count_old_126
  %t220 = add i64 0, 1
  %t218 = add i64 %t219, %t220
  %in221 = insertvalue %State %in207, i64 %t218, 0
  %t223 = add i64 0, %count_old_126
  %t224 = add i64 0, %N_old_120
  %c225 = icmp eq i64 %t223, %t224
  %t222 = zext i1 %c225 to i64
  %gc226 = icmp ne i64 %t222, 0
  br i1 %gc226, label %g227_t, label %g227_e
  g227_t:
    %t229 = add i64 0, %escapes_old_118
    %t228 = call i64 @__print_int(i64 %t229) #6
    ret i32 0
  g227_e:
  %seed_old_230 = extractvalue %State %in221, 2
  %escapes_old_231 = extractvalue %State %in221, 10
  %t1_old_232 = extractvalue %State %in221, 7
  %N_old_233 = extractvalue %State %in221, 1
  %zi_old_234 = extractvalue %State %in221, 4
  %ci_old_235 = extractvalue %State %in221, 6
  %t3_old_236 = extractvalue %State %in221, 9
  %cr_old_237 = extractvalue %State %in221, 5
  %t2_old_238 = extractvalue %State %in221, 8
  %count_old_239 = extractvalue %State %in221, 0
  %zr_old_240 = extractvalue %State %in221, 3
  %t242 = add i64 0, %seed_old_230
  %t243 = add i64 0, 3877
  %t241 = mul i64 %t242, %t243
  %in244 = insertvalue %State %in221, i64 %t241, 2
  %t246 = add i64 0, %seed_old_230
  %t247 = add i64 0, 29573
  %t245 = add i64 %t246, %t247
  %in248 = insertvalue %State %in244, i64 %t245, 2
  %t250 = add i64 0, %seed_old_230
  %t251 = add i64 0, 139968
  %t249 = srem i64 %t250, %t251
  %in252 = insertvalue %State %in248, i64 %t249, 2
  %t255 = add i64 0, %seed_old_230
  %t256 = add i64 0, 200
  %t254 = srem i64 %t255, %t256
  %t257 = add i64 0, 100
  %t253 = sub i64 %t254, %t257
  %in258 = insertvalue %State %in252, i64 %t253, 5
  %t260 = add i64 0, %seed_old_230
  %t261 = add i64 0, 3877
  %t259 = mul i64 %t260, %t261
  %in262 = insertvalue %State %in258, i64 %t259, 2
  %t264 = add i64 0, %seed_old_230
  %t265 = add i64 0, 29573
  %t263 = add i64 %t264, %t265
  %in266 = insertvalue %State %in262, i64 %t263, 2
  %t268 = add i64 0, %seed_old_230
  %t269 = add i64 0, 139968
  %t267 = srem i64 %t268, %t269
  %in270 = insertvalue %State %in266, i64 %t267, 2
  %t273 = add i64 0, %seed_old_230
  %t274 = add i64 0, 200
  %t272 = srem i64 %t273, %t274
  %t275 = add i64 0, 100
  %t271 = sub i64 %t272, %t275
  %in276 = insertvalue %State %in270, i64 %t271, 6
  %t279 = add i64 0, %zr_old_240
  %t280 = add i64 0, %cr_old_237
  %t278 = mul i64 %t279, %t280
  %t281 = add i64 0, 100
  %t277 = sdiv i64 %t278, %t281
  %in282 = insertvalue %State %in276, i64 %t277, 7
  %t285 = add i64 0, %zi_old_234
  %t286 = add i64 0, %ci_old_235
  %t284 = mul i64 %t285, %t286
  %t287 = add i64 0, 100
  %t283 = sdiv i64 %t284, %t287
  %in288 = insertvalue %State %in282, i64 %t283, 8
  %t291 = add i64 0, %zr_old_240
  %t292 = add i64 0, %ci_old_235
  %t290 = mul i64 %t291, %t292
  %t293 = add i64 0, 100
  %t289 = sdiv i64 %t290, %t293
  %in294 = insertvalue %State %in288, i64 %t289, 9
  %t296 = add i64 0, %t1_old_232
  %t297 = add i64 0, %t2_old_238
  %t295 = sub i64 %t296, %t297
  %in298 = insertvalue %State %in294, i64 %t295, 3
  %t300 = add i64 0, %t3_old_236
  %t303 = add i64 0, %zi_old_234
  %t304 = add i64 0, %cr_old_237
  %t302 = mul i64 %t303, %t304
  %t305 = add i64 0, 100
  %t301 = sdiv i64 %t302, %t305
  %t299 = add i64 %t300, %t301
  %in306 = insertvalue %State %in298, i64 %t299, 4
  %t309 = add i64 0, %escapes_old_231
  %t312 = add i64 0, %zr_old_240
  %t313 = add i64 0, %zr_old_240
  %t311 = mul i64 %t312, %t313
  %t314 = add i64 0, 100
  %t310 = sdiv i64 %t311, %t314
  %t308 = add i64 %t309, %t310
  %t317 = add i64 0, %zi_old_234
  %t318 = add i64 0, %zi_old_234
  %t316 = mul i64 %t317, %t318
  %t319 = add i64 0, 100
  %t315 = sdiv i64 %t316, %t319
  %t307 = add i64 %t308, %t315
  %in320 = insertvalue %State %in306, i64 %t307, 10
  %t323 = add i64 0, %count_old_239
  %t324 = add i64 0, 5000000
  %t322 = srem i64 %t323, %t324
  %t325 = add i64 0, 0
  %c326 = icmp eq i64 %t322, %t325
  %t321 = zext i1 %c326 to i64
  %gc327 = icmp ne i64 %t321, 0
  br i1 %gc327, label %g328_t, label %g328_e
  g328_t:
    %t330 = add i64 0, %escapes_old_231
    %t329 = call i64 @__print_int(i64 %t330) #6
    br label %g328_e
  g328_e:
  %t332 = add i64 0, %count_old_239
  %t333 = add i64 0, 1
  %t331 = add i64 %t332, %t333
  %in334 = insertvalue %State %in320, i64 %t331, 0
  %t336 = add i64 0, %count_old_239
  %t337 = add i64 0, %N_old_233
  %c338 = icmp eq i64 %t336, %t337
  %t335 = zext i1 %c338 to i64
  %gc339 = icmp ne i64 %t335, 0
  br i1 %gc339, label %g340_t, label %g340_e
  g340_t:
    %t342 = add i64 0, %escapes_old_231
    %t341 = call i64 @__print_int(i64 %t342) #6
    ret i32 0
  g340_e:
  %seed_old_343 = extractvalue %State %in334, 2
  %escapes_old_344 = extractvalue %State %in334, 10
  %t1_old_345 = extractvalue %State %in334, 7
  %N_old_346 = extractvalue %State %in334, 1
  %zi_old_347 = extractvalue %State %in334, 4
  %ci_old_348 = extractvalue %State %in334, 6
  %t3_old_349 = extractvalue %State %in334, 9
  %cr_old_350 = extractvalue %State %in334, 5
  %t2_old_351 = extractvalue %State %in334, 8
  %count_old_352 = extractvalue %State %in334, 0
  %zr_old_353 = extractvalue %State %in334, 3
  %t355 = add i64 0, %seed_old_343
  %t356 = add i64 0, 3877
  %t354 = mul i64 %t355, %t356
  %in357 = insertvalue %State %in334, i64 %t354, 2
  %t359 = add i64 0, %seed_old_343
  %t360 = add i64 0, 29573
  %t358 = add i64 %t359, %t360
  %in361 = insertvalue %State %in357, i64 %t358, 2
  %t363 = add i64 0, %seed_old_343
  %t364 = add i64 0, 139968
  %t362 = srem i64 %t363, %t364
  %in365 = insertvalue %State %in361, i64 %t362, 2
  %t368 = add i64 0, %seed_old_343
  %t369 = add i64 0, 200
  %t367 = srem i64 %t368, %t369
  %t370 = add i64 0, 100
  %t366 = sub i64 %t367, %t370
  %in371 = insertvalue %State %in365, i64 %t366, 5
  %t373 = add i64 0, %seed_old_343
  %t374 = add i64 0, 3877
  %t372 = mul i64 %t373, %t374
  %in375 = insertvalue %State %in371, i64 %t372, 2
  %t377 = add i64 0, %seed_old_343
  %t378 = add i64 0, 29573
  %t376 = add i64 %t377, %t378
  %in379 = insertvalue %State %in375, i64 %t376, 2
  %t381 = add i64 0, %seed_old_343
  %t382 = add i64 0, 139968
  %t380 = srem i64 %t381, %t382
  %in383 = insertvalue %State %in379, i64 %t380, 2
  %t386 = add i64 0, %seed_old_343
  %t387 = add i64 0, 200
  %t385 = srem i64 %t386, %t387
  %t388 = add i64 0, 100
  %t384 = sub i64 %t385, %t388
  %in389 = insertvalue %State %in383, i64 %t384, 6
  %t392 = add i64 0, %zr_old_353
  %t393 = add i64 0, %cr_old_350
  %t391 = mul i64 %t392, %t393
  %t394 = add i64 0, 100
  %t390 = sdiv i64 %t391, %t394
  %in395 = insertvalue %State %in389, i64 %t390, 7
  %t398 = add i64 0, %zi_old_347
  %t399 = add i64 0, %ci_old_348
  %t397 = mul i64 %t398, %t399
  %t400 = add i64 0, 100
  %t396 = sdiv i64 %t397, %t400
  %in401 = insertvalue %State %in395, i64 %t396, 8
  %t404 = add i64 0, %zr_old_353
  %t405 = add i64 0, %ci_old_348
  %t403 = mul i64 %t404, %t405
  %t406 = add i64 0, 100
  %t402 = sdiv i64 %t403, %t406
  %in407 = insertvalue %State %in401, i64 %t402, 9
  %t409 = add i64 0, %t1_old_345
  %t410 = add i64 0, %t2_old_351
  %t408 = sub i64 %t409, %t410
  %in411 = insertvalue %State %in407, i64 %t408, 3
  %t413 = add i64 0, %t3_old_349
  %t416 = add i64 0, %zi_old_347
  %t417 = add i64 0, %cr_old_350
  %t415 = mul i64 %t416, %t417
  %t418 = add i64 0, 100
  %t414 = sdiv i64 %t415, %t418
  %t412 = add i64 %t413, %t414
  %in419 = insertvalue %State %in411, i64 %t412, 4
  %t422 = add i64 0, %escapes_old_344
  %t425 = add i64 0, %zr_old_353
  %t426 = add i64 0, %zr_old_353
  %t424 = mul i64 %t425, %t426
  %t427 = add i64 0, 100
  %t423 = sdiv i64 %t424, %t427
  %t421 = add i64 %t422, %t423
  %t430 = add i64 0, %zi_old_347
  %t431 = add i64 0, %zi_old_347
  %t429 = mul i64 %t430, %t431
  %t432 = add i64 0, 100
  %t428 = sdiv i64 %t429, %t432
  %t420 = add i64 %t421, %t428
  %in433 = insertvalue %State %in419, i64 %t420, 10
  %t436 = add i64 0, %count_old_352
  %t437 = add i64 0, 5000000
  %t435 = srem i64 %t436, %t437
  %t438 = add i64 0, 0
  %c439 = icmp eq i64 %t435, %t438
  %t434 = zext i1 %c439 to i64
  %gc440 = icmp ne i64 %t434, 0
  br i1 %gc440, label %g441_t, label %g441_e
  g441_t:
    %t443 = add i64 0, %escapes_old_344
    %t442 = call i64 @__print_int(i64 %t443) #6
    br label %g441_e
  g441_e:
  %t445 = add i64 0, %count_old_352
  %t446 = add i64 0, 1
  %t444 = add i64 %t445, %t446
  %in447 = insertvalue %State %in433, i64 %t444, 0
  %t449 = add i64 0, %count_old_352
  %t450 = add i64 0, %N_old_346
  %c451 = icmp eq i64 %t449, %t450
  %t448 = zext i1 %c451 to i64
  %gc452 = icmp ne i64 %t448, 0
  br i1 %gc452, label %g453_t, label %g453_e
  g453_t:
    %t455 = add i64 0, %escapes_old_344
    %t454 = call i64 @__print_int(i64 %t455) #6
    ret i32 0
  g453_e:
  store %State %in447, %State* %slot_case, align 8
  br label %case_hdr
case_body1:
  %seed_old_456 = extractvalue %State %ssa_phi_case, 2
  %escapes_old_457 = extractvalue %State %ssa_phi_case, 10
  %t1_old_458 = extractvalue %State %ssa_phi_case, 7
  %N_old_459 = extractvalue %State %ssa_phi_case, 1
  %zi_old_460 = extractvalue %State %ssa_phi_case, 4
  %ci_old_461 = extractvalue %State %ssa_phi_case, 6
  %t3_old_462 = extractvalue %State %ssa_phi_case, 9
  %cr_old_463 = extractvalue %State %ssa_phi_case, 5
  %t2_old_464 = extractvalue %State %ssa_phi_case, 8
  %count_old_465 = extractvalue %State %ssa_phi_case, 0
  %zr_old_466 = extractvalue %State %ssa_phi_case, 3
  %t468 = add i64 0, %seed_old_456
  %t469 = add i64 0, 3877
  %t467 = mul i64 %t468, %t469
  %in470 = insertvalue %State %ssa_phi_case, i64 %t467, 2
  %t472 = add i64 0, %seed_old_456
  %t473 = add i64 0, 29573
  %t471 = add i64 %t472, %t473
  %in474 = insertvalue %State %in470, i64 %t471, 2
  %t476 = add i64 0, %seed_old_456
  %t477 = add i64 0, 139968
  %t475 = srem i64 %t476, %t477
  %in478 = insertvalue %State %in474, i64 %t475, 2
  %t481 = add i64 0, %seed_old_456
  %t482 = add i64 0, 200
  %t480 = srem i64 %t481, %t482
  %t483 = add i64 0, 100
  %t479 = sub i64 %t480, %t483
  %in484 = insertvalue %State %in478, i64 %t479, 5
  %t486 = add i64 0, %seed_old_456
  %t487 = add i64 0, 3877
  %t485 = mul i64 %t486, %t487
  %in488 = insertvalue %State %in484, i64 %t485, 2
  %t490 = add i64 0, %seed_old_456
  %t491 = add i64 0, 29573
  %t489 = add i64 %t490, %t491
  %in492 = insertvalue %State %in488, i64 %t489, 2
  %t494 = add i64 0, %seed_old_456
  %t495 = add i64 0, 139968
  %t493 = srem i64 %t494, %t495
  %in496 = insertvalue %State %in492, i64 %t493, 2
  %t499 = add i64 0, %seed_old_456
  %t500 = add i64 0, 200
  %t498 = srem i64 %t499, %t500
  %t501 = add i64 0, 100
  %t497 = sub i64 %t498, %t501
  %in502 = insertvalue %State %in496, i64 %t497, 6
  %t505 = add i64 0, %zr_old_466
  %t506 = add i64 0, %cr_old_463
  %t504 = mul i64 %t505, %t506
  %t507 = add i64 0, 100
  %t503 = sdiv i64 %t504, %t507
  %in508 = insertvalue %State %in502, i64 %t503, 7
  %t511 = add i64 0, %zi_old_460
  %t512 = add i64 0, %ci_old_461
  %t510 = mul i64 %t511, %t512
  %t513 = add i64 0, 100
  %t509 = sdiv i64 %t510, %t513
  %in514 = insertvalue %State %in508, i64 %t509, 8
  %t517 = add i64 0, %zr_old_466
  %t518 = add i64 0, %ci_old_461
  %t516 = mul i64 %t517, %t518
  %t519 = add i64 0, 100
  %t515 = sdiv i64 %t516, %t519
  %in520 = insertvalue %State %in514, i64 %t515, 9
  %t522 = add i64 0, %t1_old_458
  %t523 = add i64 0, %t2_old_464
  %t521 = sub i64 %t522, %t523
  %in524 = insertvalue %State %in520, i64 %t521, 3
  %t526 = add i64 0, %t3_old_462
  %t529 = add i64 0, %zi_old_460
  %t530 = add i64 0, %cr_old_463
  %t528 = mul i64 %t529, %t530
  %t531 = add i64 0, 100
  %t527 = sdiv i64 %t528, %t531
  %t525 = add i64 %t526, %t527
  %in532 = insertvalue %State %in524, i64 %t525, 4
  %t535 = add i64 0, %escapes_old_457
  %t538 = add i64 0, %zr_old_466
  %t539 = add i64 0, %zr_old_466
  %t537 = mul i64 %t538, %t539
  %t540 = add i64 0, 100
  %t536 = sdiv i64 %t537, %t540
  %t534 = add i64 %t535, %t536
  %t543 = add i64 0, %zi_old_460
  %t544 = add i64 0, %zi_old_460
  %t542 = mul i64 %t543, %t544
  %t545 = add i64 0, 100
  %t541 = sdiv i64 %t542, %t545
  %t533 = add i64 %t534, %t541
  %in546 = insertvalue %State %in532, i64 %t533, 10
  %t549 = add i64 0, %count_old_465
  %t550 = add i64 0, 5000000
  %t548 = srem i64 %t549, %t550
  %t551 = add i64 0, 0
  %c552 = icmp eq i64 %t548, %t551
  %t547 = zext i1 %c552 to i64
  %gc553 = icmp ne i64 %t547, 0
  br i1 %gc553, label %g554_t, label %g554_e
  g554_t:
    %t556 = add i64 0, %escapes_old_457
    %t555 = call i64 @__print_int(i64 %t556) #6
    br label %g554_e
  g554_e:
  %t558 = add i64 0, %count_old_465
  %t559 = add i64 0, 1
  %t557 = add i64 %t558, %t559
  %in560 = insertvalue %State %in546, i64 %t557, 0
  %t562 = add i64 0, %count_old_465
  %t563 = add i64 0, %N_old_459
  %c564 = icmp eq i64 %t562, %t563
  %t561 = zext i1 %c564 to i64
  %gc565 = icmp ne i64 %t561, 0
  br i1 %gc565, label %g566_t, label %g566_e
  g566_t:
    %t568 = add i64 0, %escapes_old_457
    %t567 = call i64 @__print_int(i64 %t568) #6
    ret i32 0
  g566_e:
  store %State %in560, %State* %slot_case, align 8
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
