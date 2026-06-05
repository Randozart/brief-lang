; ModuleID = 'benchmarks/float_math_nonzero.ll'
source_filename = "program.bv"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

%State = type { float, float, float, float, float, float, i64, i64 }

@__io_pending = external global i8, align 1
@A11 = constant float 1.000000e+00
@A21 = constant float 0x3F847AE140000000
@A02 = constant float 0x3F50624DE0000000
@Q22 = constant float 0x3FB99999A0000000
@str.0 = private unnamed_addr constant [6 x i8] c"BOUND\00", align 1
@llvm.wake_triggers = constant [1 x ptr] [ptr @__io_pending]

@A00 = alias float, ptr @A11
@A22 = alias float, ptr @A11
@A01 = alias float, ptr @A21
@A10 = alias float, ptr @A21
@Q11 = alias float, ptr @Q22
@A12 = alias float, ptr @A21
@A20 = alias float, ptr @A02
@Q00 = alias float, ptr @Q22

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
  %tr8 = load volatile i8, ptr @__io_pending, align 1
  %fdp12 = getelementptr inbounds %State, ptr %state, i64 0, i32 6
  %il13 = load i64, ptr %fdp12, align 8
  %fdp15 = getelementptr inbounds %State, ptr %state, i64 0, i32 7
  %il16 = load i64, ptr %fdp15, align 8
  %c17 = icmp slt i64 %il13, %il16
  %0 = and i8 %tr8, 1
  %il211 = load i32, ptr %state, align 4
  %fdp30 = getelementptr inbounds %State, ptr %state, i64 0, i32 1
  %il312 = load i32, ptr %fdp30, align 4
  %fdp40 = getelementptr inbounds %State, ptr %state, i64 0, i32 2
  %il413 = load i32, ptr %fdp40, align 4
  %fdp50 = getelementptr inbounds %State, ptr %state, i64 0, i32 3
  %fdp70 = getelementptr inbounds %State, ptr %state, i64 0, i32 5
  %pi784 = icmp ne i8 %0, 0
  tail call void @llvm.assume(i1 %c17)
  tail call void @llvm.assume(i1 %pi784)
  %t83 = mul i32 %il211, 1065353216
  %t91 = mul i32 %il312, 1008981770
  %t82 = add i32 %t91, %t83
  %t99 = mul i32 %il413, 981668463
  %t81 = add i32 %t82, %t99
  store i32 %t81, ptr %state, align 4
  %t120 = mul i32 %il312, 1065353216
  %reass.add = add i32 %t81, %il413
  %reass.mul = mul i32 %reass.add, 1008981770
  %t110 = add i32 %reass.mul, %t120
  store i32 %t110, ptr %fdp30, align 4
  %t141 = mul i32 %t81, 981668463
  %t149 = mul i32 %t110, 1008981770
  %t157 = mul i32 %il413, 1065353216
  %t140 = add i32 %t141, %t157
  %t139 = add i32 %t140, %t149
  store i32 %t139, ptr %fdp40, align 4
  %1 = load <2 x i32>, ptr %fdp50, align 4
  %2 = add <2 x i32> %1, <i32 1036831949, i32 1036831949>
  store <2 x i32> %2, ptr %fdp50, align 4
  %il19311 = load i32, ptr %fdp70, align 4
  %t190 = add i32 %il19311, 1036831949
  store i32 %t190, ptr %fdp70, align 4
  %t201 = add nsw i64 %il13, 1
  store i64 %t201, ptr %fdp12, align 8
  ret void
}

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: write)
define void @init_state(ptr noalias nocapture writeonly %state) local_unnamed_addr #3 {
entry:
  store <2 x float> <float 1.000000e+00, float 5.000000e-01>, ptr %state, align 4
  %ip2 = getelementptr inbounds %State, ptr %state, i64 0, i32 2
  store float 0x3FC99999A0000000, ptr %ip2, align 4
  %ip3 = getelementptr inbounds %State, ptr %state, i64 0, i32 3
  %ip7 = getelementptr inbounds %State, ptr %state, i64 0, i32 7
  tail call void @llvm.memset.p0.i64(ptr noundef nonnull align 4 dereferenceable(20) %ip3, i8 0, i64 20, i1 false)
  %t79 = tail call i64 @__get_env_int(ptr nonnull @str.0)
  store i64 %t79, ptr %ip7, align 8
  ret void
}

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(readwrite)
define void @reactor_tick(ptr noalias nocapture %state) local_unnamed_addr #4 {
entry:
  %tr83 = load volatile i8, ptr @__io_pending, align 1
  %tr8.i1 = load volatile i8, ptr @__io_pending, align 1, !noalias !1
  %fdp12.i2 = getelementptr inbounds %State, ptr %state, i64 0, i32 6
  %il13.i3 = load i64, ptr %fdp12.i2, align 8
  %fdp15.i4 = getelementptr inbounds %State, ptr %state, i64 0, i32 7
  %il16.i5 = load i64, ptr %fdp15.i4, align 8
  %c17.i6 = icmp slt i64 %il13.i3, %il16.i5
  %0 = and i8 %tr8.i1, 1
  %ri781.i = icmp ne i8 %0, 0
  %ri78.i = select i1 %c17.i6, i1 %ri781.i, i1 false
  br i1 %ri78.i, label %b0, label %ck1

b0:                                               ; preds = %entry
  tail call void @llvm.experimental.noalias.scope.decl(metadata !4)
  %tr8.i = load volatile i8, ptr @__io_pending, align 1, !noalias !4
  %1 = and i8 %tr8.i, 1
  %il211.i = load i32, ptr %state, align 4, !alias.scope !4
  %fdp30.i = getelementptr inbounds %State, ptr %state, i64 0, i32 1
  %il312.i = load i32, ptr %fdp30.i, align 4, !alias.scope !4
  %fdp40.i = getelementptr inbounds %State, ptr %state, i64 0, i32 2
  %il413.i = load i32, ptr %fdp40.i, align 4, !alias.scope !4
  %fdp50.i = getelementptr inbounds %State, ptr %state, i64 0, i32 3
  %fdp70.i = getelementptr inbounds %State, ptr %state, i64 0, i32 5
  %pi784.i = icmp ne i8 %1, 0
  tail call void @llvm.assume(i1 %pi784.i)
  %t83.i = mul i32 %il211.i, 1065353216
  %t91.i = mul i32 %il312.i, 1008981770
  %t82.i = add i32 %t91.i, %t83.i
  %t99.i = mul i32 %il413.i, 981668463
  %t81.i = add i32 %t82.i, %t99.i
  store i32 %t81.i, ptr %state, align 4, !alias.scope !4
  %t120.i = mul i32 %il312.i, 1065353216
  %reass.add = add i32 %t81.i, %il413.i
  %reass.mul = mul i32 %reass.add, 1008981770
  %t110.i = add i32 %reass.mul, %t120.i
  store i32 %t110.i, ptr %fdp30.i, align 4, !alias.scope !4
  %t141.i = mul i32 %t81.i, 981668463
  %t149.i = mul i32 %t110.i, 1008981770
  %t157.i = mul i32 %il413.i, 1065353216
  %t140.i = add i32 %t141.i, %t157.i
  %t139.i = add i32 %t140.i, %t149.i
  store i32 %t139.i, ptr %fdp40.i, align 4, !alias.scope !4
  %2 = load <2 x i32>, ptr %fdp50.i, align 4, !alias.scope !4
  %3 = add <2 x i32> %2, <i32 1036831949, i32 1036831949>
  store <2 x i32> %3, ptr %fdp50.i, align 4, !alias.scope !4
  %il19311.i = load i32, ptr %fdp70.i, align 4, !alias.scope !4
  %t190.i = add i32 %il19311.i, 1036831949
  store i32 %t190.i, ptr %fdp70.i, align 4, !alias.scope !4
  %t201.i = add nsw i64 %il13.i3, 1
  store i64 %t201.i, ptr %fdp12.i2, align 8, !alias.scope !4
  br label %ck1

ck1:                                              ; preds = %b0, %entry
  ret void
}

; Function Attrs: nofree norecurse nosync nounwind memory(readwrite)
define noundef i32 @main() local_unnamed_addr #5 {
entry:
  %t79.i = tail call i64 @__get_env_int(ptr nonnull @str.0), !noalias !7
  tail call void @__rt_init() #8
  tail call void @__rt_poll() #8
  %adjuni_tick_1059 = add i64 %t79.i, -3
  br label %tick

tick:                                             ; preds = %do_wait, %entry
  %tr85 = load volatile i8, ptr @__io_pending, align 1
  br label %uni_tick_hdr

uni_tick_hdr:                                     ; preds = %uni_tick_hdr.backedge, %tick
  %t103779 = phi i64 [ %t846, %uni_tick_hdr.backedge ], [ 0, %tick ]
  %bfr99371 = phi float [ %bfr802, %uni_tick_hdr.backedge ], [ 0x3FC99999A0000000, %tick ]
  %bfr94669 = phi float [ %bfr755, %uni_tick_hdr.backedge ], [ 5.000000e-01, %tick ]
  %bfr70867 = phi float [ %bfr70868, %uni_tick_hdr.backedge ], [ 1.000000e+00, %tick ]
  %cpuni_tick_1060 = icmp slt i64 %t103779, %adjuni_tick_1059
  br i1 %cpuni_tick_1060, label %uni_tick_body4, label %uni_tick_rem

uni_tick_rem:                                     ; preds = %uni_tick_hdr
  %cpuni_tick_1061 = icmp slt i64 %t103779, %t79.i
  br i1 %cpuni_tick_1061, label %uni_tick_body1, label %uni_tick_done

uni_tick_body4:                                   ; preds = %uni_tick_hdr
  %bfr113 = fmul fast float %bfr94669, 0x3F847AE140000000
  %bfr119 = fadd fast float %bfr113, %bfr70867
  %bfr129 = fmul fast float %bfr99371, 0x3F50624DE0000000
  %bfr135 = fadd fast float %bfr119, %bfr129
  %reass.add59 = fadd fast float %bfr99371, %bfr70867
  %reass.mul60 = fmul fast float %reass.add59, 0x3F847AE140000000
  %bfr182 = fadd fast float %reass.mul60, %bfr94669
  %bfr197 = fmul fast float %bfr70867, 0x3F50624DE0000000
  %bfr213 = fadd fast float %bfr113, %bfr197
  %bfr229 = fadd fast float %bfr213, %bfr99371
  %bfr304 = fmul fast float %bfr182, 0x3F847AE140000000
  %bfr320 = fmul fast float %bfr229, 0x3F50624DE0000000
  %bfr310 = fadd fast float %bfr320, %bfr135
  %bfr326 = fadd fast float %bfr310, %bfr304
  %reass.add61 = fadd fast float %bfr229, %bfr135
  %reass.mul62 = fmul fast float %reass.add61, 0x3F847AE140000000
  %bfr373 = fadd fast float %reass.mul62, %bfr182
  %bfr388 = fmul fast float %bfr135, 0x3F50624DE0000000
  %bfr404 = fadd fast float %bfr388, %bfr229
  %bfr420 = fadd fast float %bfr404, %bfr304
  %bfr495 = fmul fast float %bfr373, 0x3F847AE140000000
  %bfr501 = fadd fast float %bfr326, %bfr495
  %bfr511 = fmul fast float %bfr420, 0x3F50624DE0000000
  %bfr517 = fadd fast float %bfr501, %bfr511
  %reass.add63 = fadd fast float %bfr420, %bfr326
  %reass.mul64 = fmul fast float %reass.add63, 0x3F847AE140000000
  %bfr564 = fadd fast float %reass.mul64, %bfr373
  %bfr579 = fmul fast float %bfr326, 0x3F50624DE0000000
  %bfr595 = fadd fast float %bfr420, %bfr495
  %bfr611 = fadd fast float %bfr595, %bfr579
  %bfr686 = fmul fast float %bfr564, 0x3F847AE140000000
  %bfr692 = fadd fast float %bfr686, %bfr517
  %bfr702 = fmul fast float %bfr611, 0x3F50624DE0000000
  %bfr708 = fadd fast float %bfr692, %bfr702
  %reass.add65 = fadd fast float %bfr611, %bfr517
  br label %uni_tick_hdr.backedge

uni_tick_hdr.backedge:                            ; preds = %uni_tick_body4, %uni_tick_body1
  %reass.add65.sink = phi float [ %reass.add65, %uni_tick_body4 ], [ %reass.add, %uni_tick_body1 ]
  %bfr564.sink = phi float [ %bfr564, %uni_tick_body4 ], [ %bfr94669, %uni_tick_body1 ]
  %bfr517.sink = phi float [ %bfr517, %uni_tick_body4 ], [ %bfr70867, %uni_tick_body1 ]
  %bfr611.sink = phi float [ %bfr611, %uni_tick_body4 ], [ %bfr877, %uni_tick_body1 ]
  %bfr686.sink = phi float [ %bfr686, %uni_tick_body4 ], [ %bfr99371, %uni_tick_body1 ]
  %.sink = phi i64 [ 4, %uni_tick_body4 ], [ 1, %uni_tick_body1 ]
  %bfr70868 = phi float [ %bfr708, %uni_tick_body4 ], [ %bfr899, %uni_tick_body1 ]
  %reass.mul66 = fmul fast float %reass.add65.sink, 0x3F847AE140000000
  %bfr755 = fadd fast float %reass.mul66, %bfr564.sink
  %bfr770 = fmul fast float %bfr517.sink, 0x3F50624DE0000000
  %bfr786 = fadd fast float %bfr611.sink, %bfr770
  %bfr802 = fadd fast float %bfr786, %bfr686.sink
  %t846 = add i64 %t103779, %.sink
  br label %uni_tick_hdr

uni_tick_body1:                                   ; preds = %uni_tick_rem
  %bfr877 = fmul fast float %bfr94669, 0x3F847AE140000000
  %bfr883 = fadd fast float %bfr877, %bfr70867
  %bfr893 = fmul fast float %bfr99371, 0x3F50624DE0000000
  %bfr899 = fadd fast float %bfr883, %bfr893
  %reass.add = fadd fast float %bfr99371, %bfr70867
  br label %uni_tick_hdr.backedge

uni_tick_done:                                    ; preds = %uni_tick_rem
  %t1068 = icmp eq i64 %t103779, %t79.i
  %0 = bitcast float %bfr94669 to i32
  %t1073 = icmp sgt i32 %0, -1
  %t106230 = and i1 %t1073, %t1068
  br i1 %t106230, label %done, label %do_wait

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
