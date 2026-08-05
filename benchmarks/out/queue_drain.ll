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
declare i64 @__get_env_int(i8*) #1
declare i64 @__print_int(i64) #6
%State = type { i64, i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant [6 x i8] c"BOUND\00", align 1

define void @work(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
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
  %fdp14 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il15 = load i64, i64* %fdp14, align 8
  %t13 = add i64 0, %il15
  %dhp16 = inttoptr i64 %t13 to i64*
  %dlp17 = getelementptr i64, i64* %dhp16, i64 1
  %dlen18 = load i64, i64* %dlp17, align 8
  %dnl19 = sub i64 %dlen18, 1
  store i64 %dnl19, i64* %dlp17, align 8
  %t11 = add i64 0, 0 ; discard
  %fdp23 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il24 = load i64, i64* %fdp23, align 8
  %t22 = add i64 0, %il24
  %ahp25 = inttoptr i64 %t22 to i64*
  %alp26 = getelementptr i64, i64* %ahp25, i64 1
  %alen27 = load i64, i64* %alp26, align 8
  %fdp29 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il30 = load i64, i64* %fdp29, align 8
  %t28 = add i64 0, %il30
  %apos31 = add i64 %alen27, 2
  %aep32 = getelementptr i64, i64* %ahp25, i64 %apos31
  store i64 %t28, i64* %aep32, align 8
  %anl33 = add i64 %alen27, 1
  store i64 %anl33, i64* %alp26, align 8
  %t20 = ptrtoint i64* %ahp25 to i64
  %fdp36 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il37 = load i64, i64* %fdp36, align 8
  %t35 = add i64 0, %il37
  %t38 = add i64 0, 1
  %t34 = add i64 %t35, %t38
  %ap39 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  store i64 %t34, i64* %ap39, align 8
  %fdp43 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il44 = load i64, i64* %fdp43, align 8
  %t42 = add i64 0, %il44
  %t45 = add i64 0, 5000000
  %t41 = srem i64 %t42, %t45
  %t46 = add i64 0, 0
  %c47 = icmp eq i64 %t41, %t46
  %t40 = zext i1 %c47 to i64
  %gc48 = icmp ne i64 %t40, 0
  br i1 %gc48, label %g49_t, label %g49_e
  g49_t:
    %fdp52 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
    %il53 = load i64, i64* %fdp52, align 8
    %t51 = add i64 0, %il53
    %t50 = call i64 @__print_int(i64 %t51) #6
    br label %g49_e
  g49_e:
  ret void
}

define internal i1 @pre_work(%State* noalias nocapture %state) #0 {
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
  %llp5 = alloca i64, i64 3
  %ldp6 = getelementptr i64, i64* %llp5, i64 2
  %ldi7 = ptrtoint i64* %ldp6 to i64
  store i64 %ldi7, i64* %llp5, align 8
  %llp8 = getelementptr i64, i64* %llp5, i64 1
  store i64 1, i64* %llp8, align 8
  %t9 = add i64 0, 0
  %lep10 = getelementptr i64, i64* %llp5, i64 2
  store i64 %t9, i64* %lep10, align 8
  %t4 = ptrtoint i64* %llp5 to i64
  store i64 %t4, i64* %ip2, align 8
  ret void
}

define i32 @main() local_unnamed_addr #0 {
  entry:
  %state = alloca %State, align 8
  call void @init_state(%State* noalias nocapture %state)
  %gtcase_11 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %ltcase_11 = load i64, i64* %gtcase_11, align 8
  br label %case_pre
case_pre:
  %gepcase_181 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %ldcase_182 = load i64, i64* %gepcase_181, align 8
  %livcase_183 = insertvalue %State zeroinitializer, i64 %ldcase_182, 0
  %iivcase_184 = insertvalue %State %livcase_183, i64 0, 1
  %gepcase_185 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %ldcase_186 = load i64, i64* %gepcase_185, align 8
  %livcase_187 = insertvalue %State %iivcase_184, i64 %ldcase_186, 2
  %slot_case = alloca %State, align 8
  store %State %livcase_187, %State* %slot_case, align 8
  br label %case_hdr
case_hdr:
  %ssa_phi_case = load %State, %State* %slot_case, align 8
  %excase_188 = extractvalue %State %ssa_phi_case, 1
  %adjcase_189 = add i64 %ltcase_11, -3
  %cpcase_190 = icmp slt i64 %excase_188, %adjcase_189
  br i1 %cpcase_190, label %case_body4, label %case_rem
case_rem:
  %cpcase_191 = icmp slt i64 %excase_188, %ltcase_11
  br i1 %cpcase_191, label %case_body1, label %case_done
case_body4:
  %count_old_11 = extractvalue %State %ssa_phi_case, 1
  %N_old_12 = extractvalue %State %ssa_phi_case, 0
  %queue_old_13 = extractvalue %State %ssa_phi_case, 2
  %t16 = add i64 0, %queue_old_13
  %dhp17 = inttoptr i64 %t16 to i64*
  %dlp18 = getelementptr i64, i64* %dhp17, i64 1
  %dlen19 = load i64, i64* %dlp18, align 8
  %dnl20 = sub i64 %dlen19, 1
  store i64 %dnl20, i64* %dlp18, align 8
  %t14 = add i64 0, 0 ; discard
  %t23 = add i64 0, %queue_old_13
  %ahp24 = inttoptr i64 %t23 to i64*
  %alp25 = getelementptr i64, i64* %ahp24, i64 1
  %alen26 = load i64, i64* %alp25, align 8
  %t27 = add i64 0, %count_old_11
  %apos28 = add i64 %alen26, 2
  %aep29 = getelementptr i64, i64* %ahp24, i64 %apos28
  store i64 %t27, i64* %aep29, align 8
  %anl30 = add i64 %alen26, 1
  store i64 %anl30, i64* %alp25, align 8
  %t21 = ptrtoint i64* %ahp24 to i64
  %t32 = add i64 0, %count_old_11
  %t33 = add i64 0, 1
  %t31 = add i64 %t32, %t33
  %in34 = insertvalue %State %ssa_phi_case, i64 %t31, 1
  %t37 = add i64 0, %count_old_11
  %t38 = add i64 0, 5000000
  %t36 = srem i64 %t37, %t38
  %t39 = add i64 0, 0
  %c40 = icmp eq i64 %t36, %t39
  %t35 = zext i1 %c40 to i64
  %gc41 = icmp ne i64 %t35, 0
  br i1 %gc41, label %g42_t, label %g42_e
  g42_t:
    %t44 = add i64 0, %count_old_11
    %t43 = call i64 @__print_int(i64 %t44) #6
    br label %g42_e
  g42_e:
  %count_old_45 = extractvalue %State %in34, 1
  %N_old_46 = extractvalue %State %in34, 0
  %queue_old_47 = extractvalue %State %in34, 2
  %t50 = add i64 0, %queue_old_47
  %dhp51 = inttoptr i64 %t50 to i64*
  %dlp52 = getelementptr i64, i64* %dhp51, i64 1
  %dlen53 = load i64, i64* %dlp52, align 8
  %dnl54 = sub i64 %dlen53, 1
  store i64 %dnl54, i64* %dlp52, align 8
  %t48 = add i64 0, 0 ; discard
  %t57 = add i64 0, %queue_old_47
  %ahp58 = inttoptr i64 %t57 to i64*
  %alp59 = getelementptr i64, i64* %ahp58, i64 1
  %alen60 = load i64, i64* %alp59, align 8
  %t61 = add i64 0, %count_old_45
  %apos62 = add i64 %alen60, 2
  %aep63 = getelementptr i64, i64* %ahp58, i64 %apos62
  store i64 %t61, i64* %aep63, align 8
  %anl64 = add i64 %alen60, 1
  store i64 %anl64, i64* %alp59, align 8
  %t55 = ptrtoint i64* %ahp58 to i64
  %t66 = add i64 0, %count_old_45
  %t67 = add i64 0, 1
  %t65 = add i64 %t66, %t67
  %in68 = insertvalue %State %in34, i64 %t65, 1
  %t71 = add i64 0, %count_old_45
  %t72 = add i64 0, 5000000
  %t70 = srem i64 %t71, %t72
  %t73 = add i64 0, 0
  %c74 = icmp eq i64 %t70, %t73
  %t69 = zext i1 %c74 to i64
  %gc75 = icmp ne i64 %t69, 0
  br i1 %gc75, label %g76_t, label %g76_e
  g76_t:
    %t78 = add i64 0, %count_old_45
    %t77 = call i64 @__print_int(i64 %t78) #6
    br label %g76_e
  g76_e:
  %count_old_79 = extractvalue %State %in68, 1
  %N_old_80 = extractvalue %State %in68, 0
  %queue_old_81 = extractvalue %State %in68, 2
  %t84 = add i64 0, %queue_old_81
  %dhp85 = inttoptr i64 %t84 to i64*
  %dlp86 = getelementptr i64, i64* %dhp85, i64 1
  %dlen87 = load i64, i64* %dlp86, align 8
  %dnl88 = sub i64 %dlen87, 1
  store i64 %dnl88, i64* %dlp86, align 8
  %t82 = add i64 0, 0 ; discard
  %t91 = add i64 0, %queue_old_81
  %ahp92 = inttoptr i64 %t91 to i64*
  %alp93 = getelementptr i64, i64* %ahp92, i64 1
  %alen94 = load i64, i64* %alp93, align 8
  %t95 = add i64 0, %count_old_79
  %apos96 = add i64 %alen94, 2
  %aep97 = getelementptr i64, i64* %ahp92, i64 %apos96
  store i64 %t95, i64* %aep97, align 8
  %anl98 = add i64 %alen94, 1
  store i64 %anl98, i64* %alp93, align 8
  %t89 = ptrtoint i64* %ahp92 to i64
  %t100 = add i64 0, %count_old_79
  %t101 = add i64 0, 1
  %t99 = add i64 %t100, %t101
  %in102 = insertvalue %State %in68, i64 %t99, 1
  %t105 = add i64 0, %count_old_79
  %t106 = add i64 0, 5000000
  %t104 = srem i64 %t105, %t106
  %t107 = add i64 0, 0
  %c108 = icmp eq i64 %t104, %t107
  %t103 = zext i1 %c108 to i64
  %gc109 = icmp ne i64 %t103, 0
  br i1 %gc109, label %g110_t, label %g110_e
  g110_t:
    %t112 = add i64 0, %count_old_79
    %t111 = call i64 @__print_int(i64 %t112) #6
    br label %g110_e
  g110_e:
  %count_old_113 = extractvalue %State %in102, 1
  %N_old_114 = extractvalue %State %in102, 0
  %queue_old_115 = extractvalue %State %in102, 2
  %t118 = add i64 0, %queue_old_115
  %dhp119 = inttoptr i64 %t118 to i64*
  %dlp120 = getelementptr i64, i64* %dhp119, i64 1
  %dlen121 = load i64, i64* %dlp120, align 8
  %dnl122 = sub i64 %dlen121, 1
  store i64 %dnl122, i64* %dlp120, align 8
  %t116 = add i64 0, 0 ; discard
  %t125 = add i64 0, %queue_old_115
  %ahp126 = inttoptr i64 %t125 to i64*
  %alp127 = getelementptr i64, i64* %ahp126, i64 1
  %alen128 = load i64, i64* %alp127, align 8
  %t129 = add i64 0, %count_old_113
  %apos130 = add i64 %alen128, 2
  %aep131 = getelementptr i64, i64* %ahp126, i64 %apos130
  store i64 %t129, i64* %aep131, align 8
  %anl132 = add i64 %alen128, 1
  store i64 %anl132, i64* %alp127, align 8
  %t123 = ptrtoint i64* %ahp126 to i64
  %t134 = add i64 0, %count_old_113
  %t135 = add i64 0, 1
  %t133 = add i64 %t134, %t135
  %in136 = insertvalue %State %in102, i64 %t133, 1
  %t139 = add i64 0, %count_old_113
  %t140 = add i64 0, 5000000
  %t138 = srem i64 %t139, %t140
  %t141 = add i64 0, 0
  %c142 = icmp eq i64 %t138, %t141
  %t137 = zext i1 %c142 to i64
  %gc143 = icmp ne i64 %t137, 0
  br i1 %gc143, label %g144_t, label %g144_e
  g144_t:
    %t146 = add i64 0, %count_old_113
    %t145 = call i64 @__print_int(i64 %t146) #6
    br label %g144_e
  g144_e:
  store %State %in136, %State* %slot_case, align 8
  br label %case_hdr
case_body1:
  %count_old_147 = extractvalue %State %ssa_phi_case, 1
  %N_old_148 = extractvalue %State %ssa_phi_case, 0
  %queue_old_149 = extractvalue %State %ssa_phi_case, 2
  %t152 = add i64 0, %queue_old_149
  %dhp153 = inttoptr i64 %t152 to i64*
  %dlp154 = getelementptr i64, i64* %dhp153, i64 1
  %dlen155 = load i64, i64* %dlp154, align 8
  %dnl156 = sub i64 %dlen155, 1
  store i64 %dnl156, i64* %dlp154, align 8
  %t150 = add i64 0, 0 ; discard
  %t159 = add i64 0, %queue_old_149
  %ahp160 = inttoptr i64 %t159 to i64*
  %alp161 = getelementptr i64, i64* %ahp160, i64 1
  %alen162 = load i64, i64* %alp161, align 8
  %t163 = add i64 0, %count_old_147
  %apos164 = add i64 %alen162, 2
  %aep165 = getelementptr i64, i64* %ahp160, i64 %apos164
  store i64 %t163, i64* %aep165, align 8
  %anl166 = add i64 %alen162, 1
  store i64 %anl166, i64* %alp161, align 8
  %t157 = ptrtoint i64* %ahp160 to i64
  %t168 = add i64 0, %count_old_147
  %t169 = add i64 0, 1
  %t167 = add i64 %t168, %t169
  %in170 = insertvalue %State %ssa_phi_case, i64 %t167, 1
  %t173 = add i64 0, %count_old_147
  %t174 = add i64 0, 5000000
  %t172 = srem i64 %t173, %t174
  %t175 = add i64 0, 0
  %c176 = icmp eq i64 %t172, %t175
  %t171 = zext i1 %c176 to i64
  %gc177 = icmp ne i64 %t171, 0
  br i1 %gc177, label %g178_t, label %g178_e
  g178_t:
    %t180 = add i64 0, %count_old_147
    %t179 = call i64 @__print_int(i64 %t180) #6
    br label %g178_e
  g178_e:
  store %State %in170, %State* %slot_case, align 8
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
