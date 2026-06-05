; ModuleID = 'benchmarks/float_math.ll'
source_filename = "program.bv"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%State = type { float, float, float, float, float, float, float, float, float, float, float, float, i64, i64 }

@__io_pending = external global i8, align 1
@A22 = constant float 1.000000e+00
@A20 = constant float 0.000000e+00
@A01 = constant float 0x3F847AE140000000
@Q00 = constant float 0x3FB99999A0000000
@str.0 = private unnamed_addr constant [6 x i8] c"BOUND\00", align 1
@llvm.wake_triggers = constant [1 x ptr] [ptr @__io_pending]

@Q21 = alias float, ptr @A20
@A12 = alias float, ptr @A01
@Q02 = alias float, ptr @A20
@A00 = alias float, ptr @A22
@A10 = alias float, ptr @A20
@Q20 = alias float, ptr @A20
@A11 = alias float, ptr @A22
@A21 = alias float, ptr @A20
@Q22 = alias float, ptr @Q00
@Q10 = alias float, ptr @A20
@Q01 = alias float, ptr @A20
@A02 = alias float, ptr @A20
@Q11 = alias float, ptr @Q00
@Q12 = alias float, ptr @A20

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: write)
declare void @llvm.assume(i1 noundef) #0

declare void @__rt_init() local_unnamed_addr

declare void @__rt_poll() local_unnamed_addr

declare void @__rt_wait() local_unnamed_addr

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: write)
declare i64 @__get_env_int(ptr) local_unnamed_addr #1

; Function Attrs: alwaysinline mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite)
define void @tick(ptr noalias nocapture %state) local_unnamed_addr #2 {
entry:
  %tr14 = load volatile i8, ptr @__io_pending, align 1
  %fdp18 = getelementptr inbounds %State, ptr %state, i64 0, i32 12
  %il19 = load i64, ptr %fdp18, align 8
  %fdp21 = getelementptr inbounds %State, ptr %state, i64 0, i32 13
  %il22 = load i64, ptr %fdp21, align 8
  %c23 = icmp slt i64 %il19, %il22
  %0 = and i8 %tr14, 1
  %il271 = load i32, ptr %state, align 4
  %fdp36 = getelementptr inbounds %State, ptr %state, i64 0, i32 1
  %il372 = load i32, ptr %fdp36, align 4
  %fdp46 = getelementptr inbounds %State, ptr %state, i64 0, i32 2
  %fdp56 = getelementptr inbounds %State, ptr %state, i64 0, i32 3
  %fdp96 = getelementptr inbounds %State, ptr %state, i64 0, i32 7
  %fdp136 = getelementptr inbounds %State, ptr %state, i64 0, i32 11
  %pi1443 = icmp ne i8 %0, 0
  tail call void @llvm.assume(i1 %c23)
  tail call void @llvm.assume(i1 %pi1443)
  %t149 = mul i32 %il271, 1065353216
  %t157 = mul i32 %il372, 1008981770
  %t148 = add i32 %t157, %t149
  store i32 %t148, ptr %state, align 4
  %t186 = mul i32 %il372, 1065353216
  %il2005 = load i32, ptr %fdp46, align 4
  %t194 = mul i32 %il2005, 1008981770
  %t176 = add i32 %t194, %t186
  store i32 %t176, ptr %fdp36, align 4
  %t223 = mul i32 %il2005, 1065353216
  store i32 %t223, ptr %fdp46, align 4
  %il2377 = load i32, ptr %fdp56, align 4
  %t234 = add i32 %il2377, 1036831949
  store i32 %t234, ptr %fdp56, align 4
  %il2818 = load i32, ptr %fdp96, align 4
  %t278 = add i32 %il2818, 1036831949
  store i32 %t278, ptr %fdp96, align 4
  %il3259 = load i32, ptr %fdp136, align 4
  %t322 = add i32 %il3259, 1036831949
  store i32 %t322, ptr %fdp136, align 4
  %t333 = add nsw i64 %il19, 1
  store i64 %t333, ptr %fdp18, align 8
  ret void
}

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: write)
define void @init_state(ptr noalias nocapture writeonly %state) local_unnamed_addr #3 {
entry:
  %ip13 = getelementptr inbounds %State, ptr %state, i64 0, i32 13
  tail call void @llvm.memset.p0.i64(ptr noundef nonnull align 4 dereferenceable(56) %state, i8 0, i64 56, i1 false)
  %t145 = tail call i64 @__get_env_int(ptr nonnull @str.0)
  store i64 %t145, ptr %ip13, align 8
  ret void
}

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(readwrite)
define void @reactor_tick(ptr noalias nocapture %state) local_unnamed_addr #4 {
entry:
  %tr149 = load volatile i8, ptr @__io_pending, align 1
  %tr14.i1 = load volatile i8, ptr @__io_pending, align 1, !noalias !1
  %fdp18.i2 = getelementptr inbounds %State, ptr %state, i64 0, i32 12
  %il19.i3 = load i64, ptr %fdp18.i2, align 8
  %fdp21.i4 = getelementptr inbounds %State, ptr %state, i64 0, i32 13
  %il22.i5 = load i64, ptr %fdp21.i4, align 8
  %c23.i6 = icmp slt i64 %il19.i3, %il22.i5
  %0 = and i8 %tr14.i1, 1
  %ri1441.i = icmp ne i8 %0, 0
  %ri144.i = select i1 %c23.i6, i1 %ri1441.i, i1 false
  br i1 %ri144.i, label %b0, label %ck1

b0:                                               ; preds = %entry
  tail call void @llvm.experimental.noalias.scope.decl(metadata !4)
  %tr14.i = load volatile i8, ptr @__io_pending, align 1, !noalias !4
  %1 = and i8 %tr14.i, 1
  %il271.i = load i32, ptr %state, align 4, !alias.scope !4
  %fdp36.i = getelementptr inbounds %State, ptr %state, i64 0, i32 1
  %il372.i = load i32, ptr %fdp36.i, align 4, !alias.scope !4
  %fdp46.i = getelementptr inbounds %State, ptr %state, i64 0, i32 2
  %fdp56.i = getelementptr inbounds %State, ptr %state, i64 0, i32 3
  %fdp96.i = getelementptr inbounds %State, ptr %state, i64 0, i32 7
  %fdp136.i = getelementptr inbounds %State, ptr %state, i64 0, i32 11
  %pi1443.i = icmp ne i8 %1, 0
  tail call void @llvm.assume(i1 %pi1443.i)
  %t149.i = mul i32 %il271.i, 1065353216
  %t157.i = mul i32 %il372.i, 1008981770
  %t148.i = add i32 %t157.i, %t149.i
  store i32 %t148.i, ptr %state, align 4, !alias.scope !4
  %t186.i = mul i32 %il372.i, 1065353216
  %il2005.i = load i32, ptr %fdp46.i, align 4, !alias.scope !4
  %t194.i = mul i32 %il2005.i, 1008981770
  %t176.i = add i32 %t194.i, %t186.i
  store i32 %t176.i, ptr %fdp36.i, align 4, !alias.scope !4
  %t223.i = mul i32 %il2005.i, 1065353216
  store i32 %t223.i, ptr %fdp46.i, align 4, !alias.scope !4
  %il2377.i = load i32, ptr %fdp56.i, align 4, !alias.scope !4
  %t234.i = add i32 %il2377.i, 1036831949
  store i32 %t234.i, ptr %fdp56.i, align 4, !alias.scope !4
  %il2818.i = load i32, ptr %fdp96.i, align 4, !alias.scope !4
  %t278.i = add i32 %il2818.i, 1036831949
  store i32 %t278.i, ptr %fdp96.i, align 4, !alias.scope !4
  %il3259.i = load i32, ptr %fdp136.i, align 4, !alias.scope !4
  %t322.i = add i32 %il3259.i, 1036831949
  store i32 %t322.i, ptr %fdp136.i, align 4, !alias.scope !4
  %t333.i = add nsw i64 %il19.i3, 1
  store i64 %t333.i, ptr %fdp18.i2, align 8, !alias.scope !4
  br label %ck1

ck1:                                              ; preds = %b0, %entry
  ret void
}

; Function Attrs: nofree norecurse nosync nounwind memory(readwrite)
define noundef i32 @main() local_unnamed_addr #5 {
entry:
  %t145.i = tail call i64 @__get_env_int(ptr nonnull @str.0), !noalias !7
  tail call void @__rt_init() #8
  tail call void @__rt_poll() #8
  %adjuni_tick_1557 = add i64 %t145.i, -3
  br label %tick

tick:                                             ; preds = %do_wait, %entry
  %tr151 = load volatile i8, ptr @__io_pending, align 1
  br label %uni_tick_hdr

uni_tick_hdr:                                     ; preds = %uni_tick_hdr.backedge, %tick
  %t1523119 = phi i64 [ %t1248, %uni_tick_hdr.backedge ], [ 0, %tick ]
  %cpuni_tick_1558 = icmp slt i64 %t1523119, %adjuni_tick_1557
  br i1 %cpuni_tick_1558, label %uni_tick_hdr.backedge, label %uni_tick_rem

uni_tick_rem:                                     ; preds = %uni_tick_hdr
  %cpuni_tick_1559 = icmp slt i64 %t1523119, %t145.i
  br i1 %cpuni_tick_1559, label %uni_tick_hdr.backedge, label %uni_tick_done

uni_tick_hdr.backedge:                            ; preds = %uni_tick_hdr, %uni_tick_rem
  %.sink = phi i64 [ 1, %uni_tick_rem ], [ 4, %uni_tick_hdr ]
  %t1248 = add i64 %t1523119, %.sink
  br label %uni_tick_hdr

uni_tick_done:                                    ; preds = %uni_tick_rem
  %t1566 = icmp eq i64 %t1523119, %t145.i
  br i1 %t1566, label %done, label %do_wait

do_wait:                                          ; preds = %uni_tick_done
  tail call void @__rt_wait() #8
  br label %tick

done:                                             ; preds = %uni_tick_done
  ret i32 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: readwrite)
declare void @llvm.experimental.noalias.scope.decl(metadata) #6

; Function Attrs: nocallback nofree nounwind willreturn memory(argmem: write)
declare void @llvm.memset.p0.i64(ptr nocapture writeonly, i8, i64, i1 immarg) #7

attributes #0 = { mustprogress nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: write) }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: write) }
attributes #2 = { alwaysinline mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite) "disable-slp-vectorize"="true" "no-vectorize-slp"="true" }
attributes #3 = { mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: write) }
attributes #4 = { mustprogress nofree norecurse nosync nounwind willreturn memory(readwrite) }
attributes #5 = { nofree norecurse nosync nounwind memory(readwrite) "disable-slp-vectorize"="true" "no-vectorize-slp"="true" }
attributes #6 = { nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: readwrite) }
attributes #7 = { nocallback nofree nounwind willreturn memory(argmem: write) }
attributes #8 = { nounwind }

!llvm.wake_triggers = !{!0}

!0 = !{!"__io_pending"}
!1 = !{!2}
!2 = distinct !{!2, !3, !"pre_tick: %state"}
!3 = distinct !{!3, !"pre_tick"}
!4 = !{!5}
!5 = distinct !{!5, !6, !"tick: %state"}
!6 = distinct !{!6, !"tick"}
!7 = !{!8}
!8 = distinct !{!8, !9, !"init_state: %state"}
!9 = distinct !{!9, !"init_state"}
