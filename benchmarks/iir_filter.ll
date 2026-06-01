; ModuleID = 'program.ll'
source_filename = "program.bv"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

declare void @llvm.assume(i1) #1
declare void @__rt_init() local_unnamed_addr
declare void @__rt_wait() local_unnamed_addr
declare void @brief_thread_pool_init(i32, i8**) local_unnamed_addr
declare void @brief_barrier_release() local_unnamed_addr
declare void @brief_barrier_wait() local_unnamed_addr

@b2 = constant float bitcast (i32 998265562 to float)
@b1 = constant float bitcast (i32 1006654170 to float)
@total = constant i64 50000000
@b0 = constant float bitcast (i32 998265562 to float)
@a2 = constant float bitcast (i32 1062517960 to float)
@a1 = constant float bitcast (i32 3219676441 to float)
@input = constant float bitcast (i32 1065353216 to float)

%State = type { float, float, float, float, i64 }
@global_state = global %State zeroinitializer

define void @process(%State* noalias nocapture %state) local_unnamed_addr #0 alwaysinline {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  %il3 = load i64, i64* %fdp2, align 8
  %t1 = add i64 0, %il3
  %il5 = load i64, i64* @total, align 8
  %t4 = add i64 0, %il5
  %c6 = icmp slt i64 %t1, %t4
  %t0 = zext i1 %c6 to i64
  %pi7 = icmp ne i64 %t0, 0
  br i1 %pi7, label %ps9, label %pp8
  pp8:
    unreachable
  ps9:
  call void @llvm.assume(i1 %pi7)
  %il12 = load float, float* @b0, align 4
  %if13 = bitcast float %il12 to i32
  %t11 = zext i32 %if13 to i64
  %il15 = load float, float* @input, align 4
  %if16 = bitcast float %il15 to i32
  %t14 = zext i32 %if16 to i64
  %bfa17 = trunc i64 %t11 to i32
  %bfb18 = bitcast i32 %bfa17 to float
  %bfc19 = trunc i64 %t14 to i32
  %bfd20 = bitcast i32 %bfc19 to float
  %bfr21 = fmul float %bfb18, %bfd20
  %bfi22 = bitcast float %bfr21 to i32
  %t10 = zext i32 %bfi22 to i64
  ; let f0 = %t10
  %il25 = load float, float* @b1, align 4
  %if26 = bitcast float %il25 to i32
  %t24 = zext i32 %if26 to i64
  %fdp28 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il29 = load float, float* %fdp28, align 4
  %if30 = bitcast float %il29 to i32
  %t27 = zext i32 %if30 to i64
  %bfa31 = trunc i64 %t24 to i32
  %bfb32 = bitcast i32 %bfa31 to float
  %bfc33 = trunc i64 %t27 to i32
  %bfd34 = bitcast i32 %bfc33 to float
  %bfr35 = fmul float %bfb32, %bfd34
  %bfi36 = bitcast float %bfr35 to i32
  %t23 = zext i32 %bfi36 to i64
  ; let f1 = %t23
  %il39 = load float, float* @b2, align 4
  %if40 = bitcast float %il39 to i32
  %t38 = zext i32 %if40 to i64
  %fdp42 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il43 = load float, float* %fdp42, align 4
  %if44 = bitcast float %il43 to i32
  %t41 = zext i32 %if44 to i64
  %bfa45 = trunc i64 %t38 to i32
  %bfb46 = bitcast i32 %bfa45 to float
  %bfc47 = trunc i64 %t41 to i32
  %bfd48 = bitcast i32 %bfc47 to float
  %bfr49 = fmul float %bfb46, %bfd48
  %bfi50 = bitcast float %bfr49 to i32
  %t37 = zext i32 %bfi50 to i64
  ; let f2 = %t37
  %t53 = add i64 0, %t10
  %t54 = add i64 0, %t23
  %bfa55 = trunc i64 %t53 to i32
  %bfb56 = bitcast i32 %bfa55 to float
  %bfc57 = trunc i64 %t54 to i32
  %bfd58 = bitcast i32 %bfc57 to float
  %bfr59 = fadd float %bfb56, %bfd58
  %bfi60 = bitcast float %bfr59 to i32
  %t52 = zext i32 %bfi60 to i64
  %t61 = add i64 0, %t37
  %bfa62 = trunc i64 %t52 to i32
  %bfb63 = bitcast i32 %bfa62 to float
  %bfc64 = trunc i64 %t61 to i32
  %bfd65 = bitcast i32 %bfc64 to float
  %bfr66 = fadd float %bfb63, %bfd65
  %bfi67 = bitcast float %bfr66 to i32
  %t51 = zext i32 %bfi67 to i64
  ; let ff = %t51
  %il70 = load float, float* @a1, align 4
  %if71 = bitcast float %il70 to i32
  %t69 = zext i32 %if71 to i64
  %fdp73 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il74 = load float, float* %fdp73, align 4
  %if75 = bitcast float %il74 to i32
  %t72 = zext i32 %if75 to i64
  %bfa76 = trunc i64 %t69 to i32
  %bfb77 = bitcast i32 %bfa76 to float
  %bfc78 = trunc i64 %t72 to i32
  %bfd79 = bitcast i32 %bfc78 to float
  %bfr80 = fmul float %bfb77, %bfd79
  %bfi81 = bitcast float %bfr80 to i32
  %t68 = zext i32 %bfi81 to i64
  ; let fb1 = %t68
  %il84 = load float, float* @a2, align 4
  %if85 = bitcast float %il84 to i32
  %t83 = zext i32 %if85 to i64
  %fdp87 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  %il88 = load float, float* %fdp87, align 4
  %if89 = bitcast float %il88 to i32
  %t86 = zext i32 %if89 to i64
  %bfa90 = trunc i64 %t83 to i32
  %bfb91 = bitcast i32 %bfa90 to float
  %bfc92 = trunc i64 %t86 to i32
  %bfd93 = bitcast i32 %bfc92 to float
  %bfr94 = fmul float %bfb91, %bfd93
  %bfi95 = bitcast float %bfr94 to i32
  %t82 = zext i32 %bfi95 to i64
  ; let fb2 = %t82
  %t97 = add i64 0, %t68
  %t98 = add i64 0, %t82
  %bfa99 = trunc i64 %t97 to i32
  %bfb100 = bitcast i32 %bfa99 to float
  %bfc101 = trunc i64 %t98 to i32
  %bfd102 = bitcast i32 %bfc101 to float
  %bfr103 = fadd float %bfb100, %bfd102
  %bfi104 = bitcast float %bfr103 to i32
  %t96 = zext i32 %bfi104 to i64
  ; let fb = %t96
  %t106 = add i64 0, %t51
  %t107 = add i64 0, %t96
  %bfa108 = trunc i64 %t106 to i32
  %bfb109 = bitcast i32 %bfa108 to float
  %bfc110 = trunc i64 %t107 to i32
  %bfd111 = bitcast i32 %bfc110 to float
  %bfr112 = fsub float %bfb109, %bfd111
  %bfi113 = bitcast float %bfr112 to i32
  %t105 = zext i32 %bfi113 to i64
  ; let out = %t105
  %fdp115 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il116 = load float, float* %fdp115, align 4
  %if117 = bitcast float %il116 to i32
  %t114 = zext i32 %if117 to i64
  %ap118 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %ftr119 = trunc i64 %t114 to i32
  %ffl120 = bitcast i32 %ftr119 to float
  store float %ffl120, float* %ap118, align 4
  %il122 = load float, float* @input, align 4
  %if123 = bitcast float %il122 to i32
  %t121 = zext i32 %if123 to i64
  %ap124 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %ftr125 = trunc i64 %t121 to i32
  %ffl126 = bitcast i32 %ftr125 to float
  store float %ffl126, float* %ap124, align 4
  %fdp128 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il129 = load float, float* %fdp128, align 4
  %if130 = bitcast float %il129 to i32
  %t127 = zext i32 %if130 to i64
  %ap131 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  %ftr132 = trunc i64 %t127 to i32
  %ffl133 = bitcast i32 %ftr132 to float
  store float %ffl133, float* %ap131, align 4
  %t134 = add i64 0, %t105
  %ap135 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %ftr136 = trunc i64 %t134 to i32
  %ffl137 = bitcast i32 %ftr136 to float
  store float %ffl137, float* %ap135, align 4
  %fdp140 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  %il141 = load i64, i64* %fdp140, align 8
  %t139 = add i64 0, %il141
  %t142 = add i64 0, 1
  %t138 = add i64 %t139, %t142
  %ap143 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  store i64 %t138, i64* %ap143, align 8
  ret void
}

define internal i1 @pre_process(%State* noalias nocapture %state) #0 {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  %il3 = load i64, i64* %fdp2, align 8
  %t1 = add i64 0, %il3
  %il5 = load i64, i64* @total, align 8
  %t4 = add i64 0, %il5
  %c6 = icmp slt i64 %t1, %t4
  %t0 = zext i1 %c6 to i64
  %ri7 = icmp ne i64 %t0, 0
  ret i1 %ri7
}
define void @init_state() local_unnamed_addr #0 {
  entry:
  %ip0 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 0
  %ip0b = bitcast i32 0 to float
  store volatile float %ip0b, float* %ip0, align 4
  %ip1 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 1
  %ip1b = bitcast i32 0 to float
  store volatile float %ip1b, float* %ip1, align 4
  %ip2 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 2
  %ip2b = bitcast i32 0 to float
  store volatile float %ip2b, float* %ip2, align 4
  %ip3 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 3
  %ip3b = bitcast i32 0 to float
  store volatile float %ip3b, float* %ip3, align 4
  %ip4 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 4
  store volatile i64 0, i64* %ip4, align 8
  ret void
}

define i32 @main() local_unnamed_addr #0 {
  entry:
  call void @init_state()
  %ltcase_8 = load i64, i64* @total, align 8
  br label %case_hdr
case_hdr:
  %gpcase_9 = getelementptr inbounds %State, %State* @global_state, i32 0, i32 4
  %lpcase_9 = load i64, i64* %gpcase_9, align 8
  %cpcase_10 = icmp slt i64 %lpcase_9, %ltcase_8
  br i1 %cpcase_10, label %case_body, label %case_done
case_body:
  call void @process(%State* @global_state)
  br label %case_hdr
case_done:
  ret i32 0
}


attributes #0 = {
    mustprogress nofree norecurse nosync nounwind willreturn
    memory(argmem: readwrite)
}
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: write) }
attributes #2 = { mustprogress nofree norecurse nosync nounwind memory(readwrite) }
attributes #3 = { nofree norecurse nosync nounwind memory(readwrite) }
