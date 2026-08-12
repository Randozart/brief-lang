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
declare i64 @__get_env_int(i8*) #1
declare i64 @__print_float(float) #1
@Q22 = constant float bitcast (i32 1036831949 to float)
@Q10 = constant float bitcast (i32 0 to float)
@A11 = constant float bitcast (i32 1065353216 to float)
@Q20 = alias float, float* @Q10
@Q12 = alias float, float* @Q10
@Q21 = alias float, float* @Q10
@A00 = alias float, float* @A11
@A02 = alias float, float* @Q10
@A20 = alias float, float* @Q10
@A12 = constant float bitcast (i32 1008981770 to float)
@A10 = alias float, float* @Q10
@A22 = alias float, float* @A11
@Q01 = alias float, float* @Q10
@Q00 = alias float, float* @Q22
@Q02 = alias float, float* @Q10
@Q11 = alias float, float* @Q22
@A21 = alias float, float* @Q10
@A01 = alias float, float* @A12

%State = type { float, float, float, float, float, float, float, float, float, float, float, float, i64, i64 }
; %State is allocated on the stack in main() as %state = alloca %State

@str.0 = private unnamed_addr constant [6 x i8] c"BOUND\00", align 1

define void @tick(%State* noalias nocapture %state) local_unnamed_addr #4 alwaysinline {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 12
  %t1 = load i64, i64* %fdp2, align 8
  %fdp4 = getelementptr inbounds %State, %State* %state, i32 0, i32 13
  %t3 = load i64, i64* %fdp4, align 8
  %c5 = icmp slt i64 %t1, %t3
  %t6 = zext i1 %c5 to i64
  %pi7 = icmp ne i64 %t6, 0
  br i1 %pi7, label %ps9, label %pp8
  pp8:
    unreachable
  ps9:
  call void @llvm.assume(i1 %pi7)
  %il14 = load float, float* @A00, align 4
  %fdp16 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il17 = load float, float* %fdp16, align 4
  %bfr18 = fmul fast float %il14, %il17
  %il21 = load float, float* @A01, align 4
  %fdp23 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il24 = load float, float* %fdp23, align 4
  %bfr25 = fmul fast float %il21, %il24
  %bfr26 = fadd fast float %bfr18, %bfr25
  %il29 = load float, float* @A02, align 4
  %fdp31 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il32 = load float, float* %fdp31, align 4
  %bfr33 = fmul fast float %il29, %il32
  %bfr34 = fadd fast float %bfr26, %bfr33
  %ap35 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store float %bfr34, float* %ap35, align 4
  %il40 = load float, float* @A10, align 4
  %fdp42 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il43 = load float, float* %fdp42, align 4
  %bfr44 = fmul fast float %il40, %il43
  %il47 = load float, float* @A11, align 4
  %fdp49 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il50 = load float, float* %fdp49, align 4
  %bfr51 = fmul fast float %il47, %il50
  %bfr52 = fadd fast float %bfr44, %bfr51
  %il55 = load float, float* @A12, align 4
  %fdp57 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il58 = load float, float* %fdp57, align 4
  %bfr59 = fmul fast float %il55, %il58
  %bfr60 = fadd fast float %bfr52, %bfr59
  %ap61 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  store float %bfr60, float* %ap61, align 4
  %il66 = load float, float* @A20, align 4
  %fdp68 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %il69 = load float, float* %fdp68, align 4
  %bfr70 = fmul fast float %il66, %il69
  %il73 = load float, float* @A21, align 4
  %fdp75 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %il76 = load float, float* %fdp75, align 4
  %bfr77 = fmul fast float %il73, %il76
  %bfr78 = fadd fast float %bfr70, %bfr77
  %il81 = load float, float* @A22, align 4
  %fdp83 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %il84 = load float, float* %fdp83, align 4
  %bfr85 = fmul fast float %il81, %il84
  %bfr86 = fadd fast float %bfr78, %bfr85
  %ap87 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  store float %bfr86, float* %ap87, align 4
  %fdp90 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  %il91 = load float, float* %fdp90, align 4
  %il93 = load float, float* @Q00, align 4
  %bfr94 = fadd fast float %il91, %il93
  %ap95 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  store float %bfr94, float* %ap95, align 4
  %fdp98 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  %il99 = load float, float* %fdp98, align 4
  %il101 = load float, float* @Q01, align 4
  %bfr102 = fadd fast float %il99, %il101
  %ap103 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  store float %bfr102, float* %ap103, align 4
  %fdp106 = getelementptr inbounds %State, %State* %state, i32 0, i32 5
  %il107 = load float, float* %fdp106, align 4
  %il109 = load float, float* @Q02, align 4
  %bfr110 = fadd fast float %il107, %il109
  %ap111 = getelementptr inbounds %State, %State* %state, i32 0, i32 5
  store float %bfr110, float* %ap111, align 4
  %fdp114 = getelementptr inbounds %State, %State* %state, i32 0, i32 6
  %il115 = load float, float* %fdp114, align 4
  %il117 = load float, float* @Q10, align 4
  %bfr118 = fadd fast float %il115, %il117
  %ap119 = getelementptr inbounds %State, %State* %state, i32 0, i32 6
  store float %bfr118, float* %ap119, align 4
  %fdp122 = getelementptr inbounds %State, %State* %state, i32 0, i32 7
  %il123 = load float, float* %fdp122, align 4
  %il125 = load float, float* @Q11, align 4
  %bfr126 = fadd fast float %il123, %il125
  %ap127 = getelementptr inbounds %State, %State* %state, i32 0, i32 7
  store float %bfr126, float* %ap127, align 4
  %fdp130 = getelementptr inbounds %State, %State* %state, i32 0, i32 8
  %il131 = load float, float* %fdp130, align 4
  %il133 = load float, float* @Q12, align 4
  %bfr134 = fadd fast float %il131, %il133
  %ap135 = getelementptr inbounds %State, %State* %state, i32 0, i32 8
  store float %bfr134, float* %ap135, align 4
  %fdp138 = getelementptr inbounds %State, %State* %state, i32 0, i32 9
  %il139 = load float, float* %fdp138, align 4
  %il141 = load float, float* @Q20, align 4
  %bfr142 = fadd fast float %il139, %il141
  %ap143 = getelementptr inbounds %State, %State* %state, i32 0, i32 9
  store float %bfr142, float* %ap143, align 4
  %fdp146 = getelementptr inbounds %State, %State* %state, i32 0, i32 10
  %il147 = load float, float* %fdp146, align 4
  %il149 = load float, float* @Q21, align 4
  %bfr150 = fadd fast float %il147, %il149
  %ap151 = getelementptr inbounds %State, %State* %state, i32 0, i32 10
  store float %bfr150, float* %ap151, align 4
  %fdp154 = getelementptr inbounds %State, %State* %state, i32 0, i32 11
  %il155 = load float, float* %fdp154, align 4
  %il157 = load float, float* @Q22, align 4
  %bfr158 = fadd fast float %il155, %il157
  %ap159 = getelementptr inbounds %State, %State* %state, i32 0, i32 11
  store float %bfr158, float* %ap159, align 4
  %fdp162 = getelementptr inbounds %State, %State* %state, i32 0, i32 12
  %t161 = load i64, i64* %fdp162, align 8
%t164 = add i64 0, 1
  %t165 = add i64 %t161, %t164
  %ap166 = getelementptr inbounds %State, %State* %state, i32 0, i32 12
  store i64 %t165, i64* %ap166, align 8
  %fdp170 = getelementptr inbounds %State, %State* %state, i32 0, i32 12
  %t169 = load i64, i64* %fdp170, align 8
%t172 = add i64 0, 5000000
  %t168 = srem i64 %t169, %t172
%t174 = add i64 0, 0
  %c175 = icmp eq i64 %t168, %t174
  %t176 = zext i1 %c175 to i64
  %gc177 = icmp ne i64 %t176, 0
  br i1 %gc177, label %g178_t, label %g178_e
  g178_t:
    %fdp188 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
    %il189 = load float, float* %fdp188, align 4
    %fdp191 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
    %il192 = load float, float* %fdp191, align 4
    %bfr193 = fadd fast float %il189, %il192
    %fdp195 = getelementptr inbounds %State, %State* %state, i32 0, i32 5
    %il196 = load float, float* %fdp195, align 4
    %bfr197 = fadd fast float %bfr193, %il196
    %fdp199 = getelementptr inbounds %State, %State* %state, i32 0, i32 6
    %il200 = load float, float* %fdp199, align 4
    %bfr201 = fadd fast float %bfr197, %il200
    %fdp203 = getelementptr inbounds %State, %State* %state, i32 0, i32 7
    %il204 = load float, float* %fdp203, align 4
    %bfr205 = fadd fast float %bfr201, %il204
    %fdp207 = getelementptr inbounds %State, %State* %state, i32 0, i32 8
    %il208 = load float, float* %fdp207, align 4
    %bfr209 = fadd fast float %bfr205, %il208
    %fdp211 = getelementptr inbounds %State, %State* %state, i32 0, i32 9
    %il212 = load float, float* %fdp211, align 4
    %bfr213 = fadd fast float %bfr209, %il212
    %fdp215 = getelementptr inbounds %State, %State* %state, i32 0, i32 10
    %il216 = load float, float* %fdp215, align 4
    %bfr217 = fadd fast float %bfr213, %il216
    %fdp219 = getelementptr inbounds %State, %State* %state, i32 0, i32 11
    %il220 = load float, float* %fdp219, align 4
    %bfr221 = fadd fast float %bfr217, %il220
    ; let trace = %bfr221
    %fdp227 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
    %il228 = load float, float* %fdp227, align 4
    %fdp230 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
    %il231 = load float, float* %fdp230, align 4
    %bfr232 = fadd fast float %il228, %il231
    %fdp234 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
    %il235 = load float, float* %fdp234, align 4
    %bfr236 = fadd fast float %bfr232, %il235
    %bfr238 = fadd fast float %bfr236, %bfr221
    %t222 = call i64 @__print_float(float %bfr238)
    br label %g178_e
  g178_e:
  ret void
}

define internal i1 @pre_tick(%State* noalias nocapture %state) #0 {
  entry:
  %fdp2 = getelementptr inbounds %State, %State* %state, i32 0, i32 12
  %t1 = load i64, i64* %fdp2, align 8
  %fdp4 = getelementptr inbounds %State, %State* %state, i32 0, i32 13
  %t3 = load i64, i64* %fdp4, align 8
  %c5 = icmp slt i64 %t1, %t3
  %t6 = zext i1 %c5 to i64
  %ri7 = icmp ne i64 %t6, 0
  ret i1 %ri7
}
define void @init_state(%State* noalias nocapture %state) local_unnamed_addr #0 {
  entry:
  %ip0 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
%ff2 = bitcast i32 0 to float
%fi3 = bitcast float %ff2 to i32
%t1 = zext i32 %fi3 to i64
  store float %ff2, float* %ip0, align 4
  %ip1 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
%ff6 = bitcast i32 0 to float
%fi7 = bitcast float %ff6 to i32
%t5 = zext i32 %fi7 to i64
  store float %ff6, float* %ip1, align 4
  %ip2 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
%ff10 = bitcast i32 0 to float
%fi11 = bitcast float %ff10 to i32
%t9 = zext i32 %fi11 to i64
  store float %ff10, float* %ip2, align 4
  %ip3 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
%ff14 = bitcast i32 0 to float
%fi15 = bitcast float %ff14 to i32
%t13 = zext i32 %fi15 to i64
  store float %ff14, float* %ip3, align 4
  %ip4 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
%ff18 = bitcast i32 0 to float
%fi19 = bitcast float %ff18 to i32
%t17 = zext i32 %fi19 to i64
  store float %ff18, float* %ip4, align 4
  %ip5 = getelementptr inbounds %State, %State* %state, i32 0, i32 5
%ff22 = bitcast i32 0 to float
%fi23 = bitcast float %ff22 to i32
%t21 = zext i32 %fi23 to i64
  store float %ff22, float* %ip5, align 4
  %ip6 = getelementptr inbounds %State, %State* %state, i32 0, i32 6
%ff26 = bitcast i32 0 to float
%fi27 = bitcast float %ff26 to i32
%t25 = zext i32 %fi27 to i64
  store float %ff26, float* %ip6, align 4
  %ip7 = getelementptr inbounds %State, %State* %state, i32 0, i32 7
%ff30 = bitcast i32 0 to float
%fi31 = bitcast float %ff30 to i32
%t29 = zext i32 %fi31 to i64
  store float %ff30, float* %ip7, align 4
  %ip8 = getelementptr inbounds %State, %State* %state, i32 0, i32 8
%ff34 = bitcast i32 0 to float
%fi35 = bitcast float %ff34 to i32
%t33 = zext i32 %fi35 to i64
  store float %ff34, float* %ip8, align 4
  %ip9 = getelementptr inbounds %State, %State* %state, i32 0, i32 9
%ff38 = bitcast i32 0 to float
%fi39 = bitcast float %ff38 to i32
%t37 = zext i32 %fi39 to i64
  store float %ff38, float* %ip9, align 4
  %ip10 = getelementptr inbounds %State, %State* %state, i32 0, i32 10
%ff42 = bitcast i32 0 to float
%fi43 = bitcast float %ff42 to i32
%t41 = zext i32 %fi43 to i64
  store float %ff42, float* %ip10, align 4
  %ip11 = getelementptr inbounds %State, %State* %state, i32 0, i32 11
%ff46 = bitcast i32 0 to float
%fi47 = bitcast float %ff46 to i32
%t45 = zext i32 %fi47 to i64
  store float %ff46, float* %ip11, align 4
  %ip12 = getelementptr inbounds %State, %State* %state, i32 0, i32 12
%t49 = add i64 0, 0
  store i64 %t49, i64* %ip12, align 8
  %ip13 = getelementptr inbounds %State, %State* %state, i32 0, i32 13
%sp53 = getelementptr inbounds [6 x i8], [6 x i8]* @str.0, i64 0, i64 0
%t52 = ptrtoint i8* %sp53 to i64
  %fp54 = inttoptr i64 %t52 to i8*
  %t50 = call i64 @__get_env_int(i8* %fp54)
  store i64 %t50, i64* %ip13, align 8
  ret void
}

define i32 @main() local_unnamed_addr #5 {
  entry:
  %state = alloca %State, align 8
  call void @init_state(%State* noalias nocapture %state)
  br label %tick
  tick:
  %gep_p00_55 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  %p00_old_56 = load float, float* %gep_p00_55, align 4
  %gep_total_57 = getelementptr inbounds %State, %State* %state, i32 0, i32 13
  %total_old_58 = load i64, i64* %gep_total_57, align 8
  %gep_p02_59 = getelementptr inbounds %State, %State* %state, i32 0, i32 5
  %p02_old_60 = load float, float* %gep_p02_59, align 4
  %gep_p01_61 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  %p01_old_62 = load float, float* %gep_p01_61, align 4
  %gep_x0_63 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %x0_old_64 = load float, float* %gep_x0_63, align 4
  %gep_x2_65 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %x2_old_66 = load float, float* %gep_x2_65, align 4
  %gep_p11_67 = getelementptr inbounds %State, %State* %state, i32 0, i32 7
  %p11_old_68 = load float, float* %gep_p11_67, align 4
  %gep_p21_69 = getelementptr inbounds %State, %State* %state, i32 0, i32 10
  %p21_old_70 = load float, float* %gep_p21_69, align 4
  %gep_count_71 = getelementptr inbounds %State, %State* %state, i32 0, i32 12
  %count_old_72 = load i64, i64* %gep_count_71, align 8
  %gep_p12_73 = getelementptr inbounds %State, %State* %state, i32 0, i32 8
  %p12_old_74 = load float, float* %gep_p12_73, align 4
  %gep_p10_75 = getelementptr inbounds %State, %State* %state, i32 0, i32 6
  %p10_old_76 = load float, float* %gep_p10_75, align 4
  %gep_x1_77 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %x1_old_78 = load float, float* %gep_x1_77, align 4
  %gep_p20_79 = getelementptr inbounds %State, %State* %state, i32 0, i32 9
  %p20_old_80 = load float, float* %gep_p20_79, align 4
  %gep_p22_81 = getelementptr inbounds %State, %State* %state, i32 0, i32 11
  %p22_old_82 = load float, float* %gep_p22_81, align 4
  %t84 = add i64 0, %count_old_72
  %t85 = add i64 0, %total_old_58
  %c86 = icmp slt i64 %t84, %t85
  %t87 = zext i1 %c86 to i64
  %pi88 = icmp ne i64 %t87, 0
  br i1 %pi88, label %b_tick, label %s_tick
  b_tick:
  %gep_p00_89 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  %p00_old_90 = load float, float* %gep_p00_89, align 4
  %gep_total_91 = getelementptr inbounds %State, %State* %state, i32 0, i32 13
  %total_old_92 = load i64, i64* %gep_total_91, align 8
  %gep_p02_93 = getelementptr inbounds %State, %State* %state, i32 0, i32 5
  %p02_old_94 = load float, float* %gep_p02_93, align 4
  %gep_p01_95 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  %p01_old_96 = load float, float* %gep_p01_95, align 4
  %gep_x0_97 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %x0_old_98 = load float, float* %gep_x0_97, align 4
  %gep_x2_99 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  %x2_old_100 = load float, float* %gep_x2_99, align 4
  %gep_p11_101 = getelementptr inbounds %State, %State* %state, i32 0, i32 7
  %p11_old_102 = load float, float* %gep_p11_101, align 4
  %gep_p21_103 = getelementptr inbounds %State, %State* %state, i32 0, i32 10
  %p21_old_104 = load float, float* %gep_p21_103, align 4
  %gep_count_105 = getelementptr inbounds %State, %State* %state, i32 0, i32 12
  %count_old_106 = load i64, i64* %gep_count_105, align 8
  %gep_p12_107 = getelementptr inbounds %State, %State* %state, i32 0, i32 8
  %p12_old_108 = load float, float* %gep_p12_107, align 4
  %gep_p10_109 = getelementptr inbounds %State, %State* %state, i32 0, i32 6
  %p10_old_110 = load float, float* %gep_p10_109, align 4
  %gep_x1_111 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  %x1_old_112 = load float, float* %gep_x1_111, align 4
  %gep_p20_113 = getelementptr inbounds %State, %State* %state, i32 0, i32 9
  %p20_old_114 = load float, float* %gep_p20_113, align 4
  %gep_p22_115 = getelementptr inbounds %State, %State* %state, i32 0, i32 11
  %p22_old_116 = load float, float* %gep_p22_115, align 4
  %il121 = load float, float* @A00, align 4
  %bfr123 = fmul fast float %il121, %x0_old_98
  %il126 = load float, float* @A01, align 4
  %bfr128 = fmul fast float %il126, %x1_old_112
  %bfr129 = fadd fast float %bfr123, %bfr128
  %il132 = load float, float* @A02, align 4
  %bfr134 = fmul fast float %il132, %x2_old_100
  %bfr135 = fadd fast float %bfr129, %bfr134
  %ap136 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  store float %bfr135, float* %ap136, align 4
  %il141 = load float, float* @A10, align 4
  %bfr143 = fmul fast float %il141, %x0_old_98
  %il146 = load float, float* @A11, align 4
  %bfr148 = fmul fast float %il146, %x1_old_112
  %bfr149 = fadd fast float %bfr143, %bfr148
  %il152 = load float, float* @A12, align 4
  %bfr154 = fmul fast float %il152, %x2_old_100
  %bfr155 = fadd fast float %bfr149, %bfr154
  %ap156 = getelementptr inbounds %State, %State* %state, i32 0, i32 1
  store float %bfr155, float* %ap156, align 4
  %il161 = load float, float* @A20, align 4
  %bfr163 = fmul fast float %il161, %x0_old_98
  %il166 = load float, float* @A21, align 4
  %bfr168 = fmul fast float %il166, %x1_old_112
  %bfr169 = fadd fast float %bfr163, %bfr168
  %il172 = load float, float* @A22, align 4
  %bfr174 = fmul fast float %il172, %x2_old_100
  %bfr175 = fadd fast float %bfr169, %bfr174
  %ap176 = getelementptr inbounds %State, %State* %state, i32 0, i32 2
  store float %bfr175, float* %ap176, align 4
  %il180 = load float, float* @Q00, align 4
  %bfr181 = fadd fast float %p00_old_90, %il180
  %ap182 = getelementptr inbounds %State, %State* %state, i32 0, i32 3
  store float %bfr181, float* %ap182, align 4
  %il186 = load float, float* @Q01, align 4
  %bfr187 = fadd fast float %p01_old_96, %il186
  %ap188 = getelementptr inbounds %State, %State* %state, i32 0, i32 4
  store float %bfr187, float* %ap188, align 4
  %il192 = load float, float* @Q02, align 4
  %bfr193 = fadd fast float %p02_old_94, %il192
  %ap194 = getelementptr inbounds %State, %State* %state, i32 0, i32 5
  store float %bfr193, float* %ap194, align 4
  %il198 = load float, float* @Q10, align 4
  %bfr199 = fadd fast float %p10_old_110, %il198
  %ap200 = getelementptr inbounds %State, %State* %state, i32 0, i32 6
  store float %bfr199, float* %ap200, align 4
  %il204 = load float, float* @Q11, align 4
  %bfr205 = fadd fast float %p11_old_102, %il204
  %ap206 = getelementptr inbounds %State, %State* %state, i32 0, i32 7
  store float %bfr205, float* %ap206, align 4
  %il210 = load float, float* @Q12, align 4
  %bfr211 = fadd fast float %p12_old_108, %il210
  %ap212 = getelementptr inbounds %State, %State* %state, i32 0, i32 8
  store float %bfr211, float* %ap212, align 4
  %il216 = load float, float* @Q20, align 4
  %bfr217 = fadd fast float %p20_old_114, %il216
  %ap218 = getelementptr inbounds %State, %State* %state, i32 0, i32 9
  store float %bfr217, float* %ap218, align 4
  %il222 = load float, float* @Q21, align 4
  %bfr223 = fadd fast float %p21_old_104, %il222
  %ap224 = getelementptr inbounds %State, %State* %state, i32 0, i32 10
  store float %bfr223, float* %ap224, align 4
  %il228 = load float, float* @Q22, align 4
  %bfr229 = fadd fast float %p22_old_116, %il228
  %ap230 = getelementptr inbounds %State, %State* %state, i32 0, i32 11
  store float %bfr229, float* %ap230, align 4
  %t232 = add i64 0, %count_old_106
%t234 = add i64 0, 1
  %t235 = add i64 %t232, %t234
  %ap236 = getelementptr inbounds %State, %State* %state, i32 0, i32 12
  store i64 %t235, i64* %ap236, align 8
  %t239 = add i64 0, %count_old_106
%t241 = add i64 0, 5000000
  %t238 = srem i64 %t239, %t241
%t243 = add i64 0, 0
  %c244 = icmp eq i64 %t238, %t243
  %t245 = zext i1 %c244 to i64
  %gc246 = icmp ne i64 %t245, 0
  br i1 %gc246, label %g247_t, label %g247_e
  g247_t:
    %bfr258 = fadd fast float %p00_old_90, %p01_old_96
    %bfr260 = fadd fast float %bfr258, %p02_old_94
    %bfr262 = fadd fast float %bfr260, %p10_old_110
    %bfr264 = fadd fast float %bfr262, %p11_old_102
    %bfr266 = fadd fast float %bfr264, %p12_old_108
    %bfr268 = fadd fast float %bfr266, %p20_old_114
    %bfr270 = fadd fast float %bfr268, %p21_old_104
    %bfr272 = fadd fast float %bfr270, %p22_old_116
    ; let trace = %bfr272
    %bfr279 = fadd fast float %x0_old_98, %x1_old_112
    %bfr281 = fadd fast float %bfr279, %x2_old_100
    %bfr283 = fadd fast float %bfr281, %bfr272
    %t273 = call i64 @__print_float(float %bfr283)
    br label %g247_e
  g247_e:
  br label %s_tick
  s_tick:
  %gep_exit_287 = getelementptr inbounds %State, %State* %state, i32 0, i32 12
  %t286 = load i64, i64* %gep_exit_287, align 8
  %gep_exit_289 = getelementptr inbounds %State, %State* %state, i32 0, i32 13
  %t288 = load i64, i64* %gep_exit_289, align 8
  %t290 = icmp eq i64 %t286, %t288
  %t285 = zext i1 %t290 to i64
  %gep_exit_293 = getelementptr inbounds %State, %State* %state, i32 0, i32 0
  %t292 = load i64, i64* %gep_exit_293, align 8
  %t294 = add i64 0, 0 ; unsupported exit expr
  %t295 = icmp sge i64 %t292, %t294
  %t291 = zext i1 %t295 to i64
  %t284 = and i64 %t285, %t291
  %t296 = trunc i64 %t284 to i1
  br i1 %t296, label %done, label %tick
  done:
  ret i32 0
}


attributes #0 = {
    mustprogress nofree norecurse nosync nounwind willreturn
    memory(argmem: readwrite)
}
attributes #1 = { nocallback nofree nosync nounwind willreturn }
attributes #2 = { mustprogress nofree norecurse nosync nounwind memory(readwrite) }
attributes #3 = { nofree norecurse nosync nounwind memory(readwrite) }
attributes #4 = {
    mustprogress nofree norecurse nosync nounwind willreturn
    memory(argmem: readwrite)
    "disable-slp-vectorize"="true" "no-vectorize-slp"="true"
}
attributes #5 = {
    nofree norecurse nosync nounwind memory(readwrite)
    "disable-slp-vectorize"="true" "no-vectorize-slp"="true"
}
