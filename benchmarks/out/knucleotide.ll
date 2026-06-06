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
@IM = constant i64 139968
@MASK = constant i64 63
@IC = constant i64 29573
@IA = constant i64 3877

%State = type { i64, i64, i64, i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant [6 x i8] c"BOUND\00", align 1

define void @kn(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
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
  %fdp32 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  %il33 = load i64, i64* %fdp32, align 8
  %t31 = add i64 0, %il33
  %t34 = add i64 0, 2
  %t30 = shl i64 %t31, %t34
  %fdp37 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il38 = load i64, i64* %fdp37, align 8
  %t36 = add i64 0, %il38
  %t39 = add i64 0, 3
  %t35 = and i64 %t36, %t39
  %t29 = or i64 %t30, %t35
  %ap40 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  store i64 %t29, i64* %ap40, align 8
  %fdp43 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  %il44 = load i64, i64* %fdp43, align 8
  %t42 = add i64 0, %il44
  %t45 = add i64 0, 63
  %t41 = and i64 %t42, %t45
  %ap46 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  store i64 %t41, i64* %ap46, align 8
  %fdp49 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  %il50 = load i64, i64* %fdp49, align 8
  %t48 = add i64 0, %il50
  %fdp53 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  %il54 = load i64, i64* %fdp53, align 8
  %t52 = add i64 0, %il54
  %t55 = add i64 0, 13
  %t51 = srem i64 %t52, %t55
  %t47 = add i64 %t48, %t51
  %ap56 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  store i64 %t47, i64* %ap56, align 8
  %fdp60 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il61 = load i64, i64* %fdp60, align 8
  %t59 = add i64 0, %il61
  %t62 = add i64 0, 5000000
  %t58 = srem i64 %t59, %t62
  %t63 = add i64 0, 0
  %c64 = icmp eq i64 %t58, %t63
  %t57 = zext i1 %c64 to i64
  %gc65 = icmp ne i64 %t57, 0
  br i1 %gc65, label %g66_t, label %g66_e
  g66_t:
    %fdp69 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
    %il70 = load i64, i64* %fdp69, align 8
    %t68 = add i64 0, %il70
    %t67 = call i64 @__print_int(i64 %t68) #6
    br label %g66_e
  g66_e:
  %fdp73 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il74 = load i64, i64* %fdp73, align 8
  %t72 = add i64 0, %il74
  %t75 = add i64 0, 1
  %t71 = add i64 %t72, %t75
  %ap76 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store i64 %t71, i64* %ap76, align 8
  %fdp79 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il80 = load i64, i64* %fdp79, align 8
  %t78 = add i64 0, %il80
  %fdp82 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il83 = load i64, i64* %fdp82, align 8
  %t81 = add i64 0, %il83
  %c84 = icmp eq i64 %t78, %t81
  %t77 = zext i1 %c84 to i64
  %gc85 = icmp ne i64 %t77, 0
  br i1 %gc85, label %g86_t, label %g86_e
  g86_t:
    %fdp89 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
    %il90 = load i64, i64* %fdp89, align 8
    %t88 = add i64 0, %il90
    %t87 = call i64 @__print_int(i64 %t88) #6
    ret void
  g86_e:
  ret void
}

define internal i1 @pre_kn(%State* noalias nocapture %state) #0 {
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
  %iivcase_289 = insertvalue %State zeroinitializer, i64 0, 0
  %gepcase_290 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %ldcase_291 = load i64, i64* %gepcase_290, align 8
  %livcase_292 = insertvalue %State %iivcase_289, i64 %ldcase_291, 1
  %iivcase_293 = insertvalue %State %livcase_292, i64 42, 2
  %iivcase_294 = insertvalue %State %iivcase_293, i64 0, 3
  %iivcase_295 = insertvalue %State %iivcase_294, i64 0, 4
  %slot_case = alloca %State, align 8
  store %State %iivcase_295, %State* %slot_case, align 8
  br label %case_hdr
case_hdr:
  %ssa_phi_case = load %State, %State* %slot_case, align 8
  %excase_296 = extractvalue %State %ssa_phi_case, 0
  %adjcase_297 = add i64 %ltcase_4, -3
  %cpcase_298 = icmp slt i64 %excase_296, %adjcase_297
  br i1 %cpcase_298, label %case_body4, label %case_rem
case_rem:
  %cpcase_299 = icmp slt i64 %excase_296, %ltcase_4
  br i1 %cpcase_299, label %case_body1, label %case_done
case_body4:
  %count_old_4 = extractvalue %State %ssa_phi_case, 0
  %N_old_5 = extractvalue %State %ssa_phi_case, 1
  %seed_old_6 = extractvalue %State %ssa_phi_case, 2
  %hash_old_7 = extractvalue %State %ssa_phi_case, 3
  %chksum_old_8 = extractvalue %State %ssa_phi_case, 4
  %t10 = add i64 0, %seed_old_6
  %t11 = add i64 0, 3877
  %t9 = mul i64 %t10, %t11
  %in12 = insertvalue %State %ssa_phi_case, i64 %t9, 2
  %t14 = add i64 0, %seed_old_6
  %t15 = add i64 0, 29573
  %t13 = add i64 %t14, %t15
  %in16 = insertvalue %State %in12, i64 %t13, 2
  %t18 = add i64 0, %seed_old_6
  %t19 = add i64 0, 139968
  %t17 = srem i64 %t18, %t19
  %in20 = insertvalue %State %in16, i64 %t17, 2
  %t23 = add i64 0, %hash_old_7
  %t24 = add i64 0, 2
  %t22 = shl i64 %t23, %t24
  %t26 = add i64 0, %seed_old_6
  %t27 = add i64 0, 3
  %t25 = and i64 %t26, %t27
  %t21 = or i64 %t22, %t25
  %in28 = insertvalue %State %in20, i64 %t21, 3
  %t30 = add i64 0, %hash_old_7
  %t31 = add i64 0, 63
  %t29 = and i64 %t30, %t31
  %in32 = insertvalue %State %in28, i64 %t29, 3
  %t34 = add i64 0, %chksum_old_8
  %t36 = add i64 0, %hash_old_7
  %t37 = add i64 0, 13
  %t35 = srem i64 %t36, %t37
  %t33 = add i64 %t34, %t35
  %in38 = insertvalue %State %in32, i64 %t33, 4
  %t41 = add i64 0, %count_old_4
  %t42 = add i64 0, 5000000
  %t40 = srem i64 %t41, %t42
  %t43 = add i64 0, 0
  %c44 = icmp eq i64 %t40, %t43
  %t39 = zext i1 %c44 to i64
  %gc45 = icmp ne i64 %t39, 0
  br i1 %gc45, label %g46_t, label %g46_e
  g46_t:
    %t48 = add i64 0, %chksum_old_8
    %t47 = call i64 @__print_int(i64 %t48) #6
    br label %g46_e
  g46_e:
  %t50 = add i64 0, %count_old_4
  %t51 = add i64 0, 1
  %t49 = add i64 %t50, %t51
  %in52 = insertvalue %State %in38, i64 %t49, 0
  %t54 = add i64 0, %count_old_4
  %t55 = add i64 0, %N_old_5
  %c56 = icmp eq i64 %t54, %t55
  %t53 = zext i1 %c56 to i64
  %gc57 = icmp ne i64 %t53, 0
  br i1 %gc57, label %g58_t, label %g58_e
  g58_t:
    %t60 = add i64 0, %chksum_old_8
    %t59 = call i64 @__print_int(i64 %t60) #6
    ret i32 0
  g58_e:
  %count_old_61 = extractvalue %State %in52, 0
  %N_old_62 = extractvalue %State %in52, 1
  %seed_old_63 = extractvalue %State %in52, 2
  %hash_old_64 = extractvalue %State %in52, 3
  %chksum_old_65 = extractvalue %State %in52, 4
  %t67 = add i64 0, %seed_old_63
  %t68 = add i64 0, 3877
  %t66 = mul i64 %t67, %t68
  %in69 = insertvalue %State %in52, i64 %t66, 2
  %t71 = add i64 0, %seed_old_63
  %t72 = add i64 0, 29573
  %t70 = add i64 %t71, %t72
  %in73 = insertvalue %State %in69, i64 %t70, 2
  %t75 = add i64 0, %seed_old_63
  %t76 = add i64 0, 139968
  %t74 = srem i64 %t75, %t76
  %in77 = insertvalue %State %in73, i64 %t74, 2
  %t80 = add i64 0, %hash_old_64
  %t81 = add i64 0, 2
  %t79 = shl i64 %t80, %t81
  %t83 = add i64 0, %seed_old_63
  %t84 = add i64 0, 3
  %t82 = and i64 %t83, %t84
  %t78 = or i64 %t79, %t82
  %in85 = insertvalue %State %in77, i64 %t78, 3
  %t87 = add i64 0, %hash_old_64
  %t88 = add i64 0, 63
  %t86 = and i64 %t87, %t88
  %in89 = insertvalue %State %in85, i64 %t86, 3
  %t91 = add i64 0, %chksum_old_65
  %t93 = add i64 0, %hash_old_64
  %t94 = add i64 0, 13
  %t92 = srem i64 %t93, %t94
  %t90 = add i64 %t91, %t92
  %in95 = insertvalue %State %in89, i64 %t90, 4
  %t98 = add i64 0, %count_old_61
  %t99 = add i64 0, 5000000
  %t97 = srem i64 %t98, %t99
  %t100 = add i64 0, 0
  %c101 = icmp eq i64 %t97, %t100
  %t96 = zext i1 %c101 to i64
  %gc102 = icmp ne i64 %t96, 0
  br i1 %gc102, label %g103_t, label %g103_e
  g103_t:
    %t105 = add i64 0, %chksum_old_65
    %t104 = call i64 @__print_int(i64 %t105) #6
    br label %g103_e
  g103_e:
  %t107 = add i64 0, %count_old_61
  %t108 = add i64 0, 1
  %t106 = add i64 %t107, %t108
  %in109 = insertvalue %State %in95, i64 %t106, 0
  %t111 = add i64 0, %count_old_61
  %t112 = add i64 0, %N_old_62
  %c113 = icmp eq i64 %t111, %t112
  %t110 = zext i1 %c113 to i64
  %gc114 = icmp ne i64 %t110, 0
  br i1 %gc114, label %g115_t, label %g115_e
  g115_t:
    %t117 = add i64 0, %chksum_old_65
    %t116 = call i64 @__print_int(i64 %t117) #6
    ret i32 0
  g115_e:
  %count_old_118 = extractvalue %State %in109, 0
  %N_old_119 = extractvalue %State %in109, 1
  %seed_old_120 = extractvalue %State %in109, 2
  %hash_old_121 = extractvalue %State %in109, 3
  %chksum_old_122 = extractvalue %State %in109, 4
  %t124 = add i64 0, %seed_old_120
  %t125 = add i64 0, 3877
  %t123 = mul i64 %t124, %t125
  %in126 = insertvalue %State %in109, i64 %t123, 2
  %t128 = add i64 0, %seed_old_120
  %t129 = add i64 0, 29573
  %t127 = add i64 %t128, %t129
  %in130 = insertvalue %State %in126, i64 %t127, 2
  %t132 = add i64 0, %seed_old_120
  %t133 = add i64 0, 139968
  %t131 = srem i64 %t132, %t133
  %in134 = insertvalue %State %in130, i64 %t131, 2
  %t137 = add i64 0, %hash_old_121
  %t138 = add i64 0, 2
  %t136 = shl i64 %t137, %t138
  %t140 = add i64 0, %seed_old_120
  %t141 = add i64 0, 3
  %t139 = and i64 %t140, %t141
  %t135 = or i64 %t136, %t139
  %in142 = insertvalue %State %in134, i64 %t135, 3
  %t144 = add i64 0, %hash_old_121
  %t145 = add i64 0, 63
  %t143 = and i64 %t144, %t145
  %in146 = insertvalue %State %in142, i64 %t143, 3
  %t148 = add i64 0, %chksum_old_122
  %t150 = add i64 0, %hash_old_121
  %t151 = add i64 0, 13
  %t149 = srem i64 %t150, %t151
  %t147 = add i64 %t148, %t149
  %in152 = insertvalue %State %in146, i64 %t147, 4
  %t155 = add i64 0, %count_old_118
  %t156 = add i64 0, 5000000
  %t154 = srem i64 %t155, %t156
  %t157 = add i64 0, 0
  %c158 = icmp eq i64 %t154, %t157
  %t153 = zext i1 %c158 to i64
  %gc159 = icmp ne i64 %t153, 0
  br i1 %gc159, label %g160_t, label %g160_e
  g160_t:
    %t162 = add i64 0, %chksum_old_122
    %t161 = call i64 @__print_int(i64 %t162) #6
    br label %g160_e
  g160_e:
  %t164 = add i64 0, %count_old_118
  %t165 = add i64 0, 1
  %t163 = add i64 %t164, %t165
  %in166 = insertvalue %State %in152, i64 %t163, 0
  %t168 = add i64 0, %count_old_118
  %t169 = add i64 0, %N_old_119
  %c170 = icmp eq i64 %t168, %t169
  %t167 = zext i1 %c170 to i64
  %gc171 = icmp ne i64 %t167, 0
  br i1 %gc171, label %g172_t, label %g172_e
  g172_t:
    %t174 = add i64 0, %chksum_old_122
    %t173 = call i64 @__print_int(i64 %t174) #6
    ret i32 0
  g172_e:
  %count_old_175 = extractvalue %State %in166, 0
  %N_old_176 = extractvalue %State %in166, 1
  %seed_old_177 = extractvalue %State %in166, 2
  %hash_old_178 = extractvalue %State %in166, 3
  %chksum_old_179 = extractvalue %State %in166, 4
  %t181 = add i64 0, %seed_old_177
  %t182 = add i64 0, 3877
  %t180 = mul i64 %t181, %t182
  %in183 = insertvalue %State %in166, i64 %t180, 2
  %t185 = add i64 0, %seed_old_177
  %t186 = add i64 0, 29573
  %t184 = add i64 %t185, %t186
  %in187 = insertvalue %State %in183, i64 %t184, 2
  %t189 = add i64 0, %seed_old_177
  %t190 = add i64 0, 139968
  %t188 = srem i64 %t189, %t190
  %in191 = insertvalue %State %in187, i64 %t188, 2
  %t194 = add i64 0, %hash_old_178
  %t195 = add i64 0, 2
  %t193 = shl i64 %t194, %t195
  %t197 = add i64 0, %seed_old_177
  %t198 = add i64 0, 3
  %t196 = and i64 %t197, %t198
  %t192 = or i64 %t193, %t196
  %in199 = insertvalue %State %in191, i64 %t192, 3
  %t201 = add i64 0, %hash_old_178
  %t202 = add i64 0, 63
  %t200 = and i64 %t201, %t202
  %in203 = insertvalue %State %in199, i64 %t200, 3
  %t205 = add i64 0, %chksum_old_179
  %t207 = add i64 0, %hash_old_178
  %t208 = add i64 0, 13
  %t206 = srem i64 %t207, %t208
  %t204 = add i64 %t205, %t206
  %in209 = insertvalue %State %in203, i64 %t204, 4
  %t212 = add i64 0, %count_old_175
  %t213 = add i64 0, 5000000
  %t211 = srem i64 %t212, %t213
  %t214 = add i64 0, 0
  %c215 = icmp eq i64 %t211, %t214
  %t210 = zext i1 %c215 to i64
  %gc216 = icmp ne i64 %t210, 0
  br i1 %gc216, label %g217_t, label %g217_e
  g217_t:
    %t219 = add i64 0, %chksum_old_179
    %t218 = call i64 @__print_int(i64 %t219) #6
    br label %g217_e
  g217_e:
  %t221 = add i64 0, %count_old_175
  %t222 = add i64 0, 1
  %t220 = add i64 %t221, %t222
  %in223 = insertvalue %State %in209, i64 %t220, 0
  %t225 = add i64 0, %count_old_175
  %t226 = add i64 0, %N_old_176
  %c227 = icmp eq i64 %t225, %t226
  %t224 = zext i1 %c227 to i64
  %gc228 = icmp ne i64 %t224, 0
  br i1 %gc228, label %g229_t, label %g229_e
  g229_t:
    %t231 = add i64 0, %chksum_old_179
    %t230 = call i64 @__print_int(i64 %t231) #6
    ret i32 0
  g229_e:
  store %State %in223, %State* %slot_case, align 8
  br label %case_hdr
case_body1:
  %count_old_232 = extractvalue %State %ssa_phi_case, 0
  %N_old_233 = extractvalue %State %ssa_phi_case, 1
  %seed_old_234 = extractvalue %State %ssa_phi_case, 2
  %hash_old_235 = extractvalue %State %ssa_phi_case, 3
  %chksum_old_236 = extractvalue %State %ssa_phi_case, 4
  %t238 = add i64 0, %seed_old_234
  %t239 = add i64 0, 3877
  %t237 = mul i64 %t238, %t239
  %in240 = insertvalue %State %ssa_phi_case, i64 %t237, 2
  %t242 = add i64 0, %seed_old_234
  %t243 = add i64 0, 29573
  %t241 = add i64 %t242, %t243
  %in244 = insertvalue %State %in240, i64 %t241, 2
  %t246 = add i64 0, %seed_old_234
  %t247 = add i64 0, 139968
  %t245 = srem i64 %t246, %t247
  %in248 = insertvalue %State %in244, i64 %t245, 2
  %t251 = add i64 0, %hash_old_235
  %t252 = add i64 0, 2
  %t250 = shl i64 %t251, %t252
  %t254 = add i64 0, %seed_old_234
  %t255 = add i64 0, 3
  %t253 = and i64 %t254, %t255
  %t249 = or i64 %t250, %t253
  %in256 = insertvalue %State %in248, i64 %t249, 3
  %t258 = add i64 0, %hash_old_235
  %t259 = add i64 0, 63
  %t257 = and i64 %t258, %t259
  %in260 = insertvalue %State %in256, i64 %t257, 3
  %t262 = add i64 0, %chksum_old_236
  %t264 = add i64 0, %hash_old_235
  %t265 = add i64 0, 13
  %t263 = srem i64 %t264, %t265
  %t261 = add i64 %t262, %t263
  %in266 = insertvalue %State %in260, i64 %t261, 4
  %t269 = add i64 0, %count_old_232
  %t270 = add i64 0, 5000000
  %t268 = srem i64 %t269, %t270
  %t271 = add i64 0, 0
  %c272 = icmp eq i64 %t268, %t271
  %t267 = zext i1 %c272 to i64
  %gc273 = icmp ne i64 %t267, 0
  br i1 %gc273, label %g274_t, label %g274_e
  g274_t:
    %t276 = add i64 0, %chksum_old_236
    %t275 = call i64 @__print_int(i64 %t276) #6
    br label %g274_e
  g274_e:
  %t278 = add i64 0, %count_old_232
  %t279 = add i64 0, 1
  %t277 = add i64 %t278, %t279
  %in280 = insertvalue %State %in266, i64 %t277, 0
  %t282 = add i64 0, %count_old_232
  %t283 = add i64 0, %N_old_233
  %c284 = icmp eq i64 %t282, %t283
  %t281 = zext i1 %c284 to i64
  %gc285 = icmp ne i64 %t281, 0
  br i1 %gc285, label %g286_t, label %g286_e
  g286_t:
    %t288 = add i64 0, %chksum_old_236
    %t287 = call i64 @__print_int(i64 %t288) #6
    ret i32 0
  g286_e:
  store %State %in280, %State* %slot_case, align 8
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
