; ModuleID = 'benchmarks/print_loop.ll'
source_filename = "program.bv"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-pc-linux-gnu"

@__io_pending = external global i8, align 1
@N = local_unnamed_addr constant i64 50000000
@print_interval = local_unnamed_addr constant i64 100000
@llvm.wake_triggers = constant [1 x ptr] [ptr @__io_pending]

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: write)
declare void @llvm.assume(i1 noundef) #0

declare void @__rt_init() local_unnamed_addr

declare void @__rt_poll() local_unnamed_addr

declare void @__rt_wait() local_unnamed_addr

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: write)
declare i64 @__print_int(i64) local_unnamed_addr #1

; Function Attrs: alwaysinline mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite)
define void @work(ptr noalias nocapture %state) local_unnamed_addr #2 {
entry:
  %tr2 = load volatile i8, ptr @__io_pending, align 1
  %il7 = load i64, ptr %state, align 8
  %c9 = icmp slt i64 %il7, 50000000
  %0 = and i8 %tr2, 1
  %pi101 = icmp ne i8 %0, 0
  tail call void @llvm.assume(i1 %c9)
  tail call void @llvm.assume(i1 %pi101)
  %t13 = add nsw i64 %il7, 1
  store i64 %t13, ptr %state, align 8
  %t20 = srem i64 %t13, 100000
  %c26 = icmp eq i64 %t20, 0
  br i1 %c26, label %g28_t, label %g28_e

g28_t:                                            ; preds = %entry
  %t29 = tail call i64 @__print_int(i64 %t13)
  br label %g28_e

g28_e:                                            ; preds = %g28_t, %entry
  ret void
}

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: write)
define void @init_state(ptr noalias nocapture writeonly %state) local_unnamed_addr #3 {
entry:
  store i64 0, ptr %state, align 8
  ret void
}

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(readwrite)
define void @reactor_tick(ptr noalias nocapture %state) local_unnamed_addr #4 {
entry:
  %tr11 = load volatile i8, ptr @__io_pending, align 1
  %tr2.i1 = load volatile i8, ptr @__io_pending, align 1, !noalias !1
  %il7.i2 = load i64, ptr %state, align 8
  %c9.i3 = icmp slt i64 %il7.i2, 50000000
  %0 = and i8 %tr2.i1, 1
  %ri101.i = icmp ne i8 %0, 0
  %ri10.i = select i1 %c9.i3, i1 %ri101.i, i1 false
  br i1 %ri10.i, label %b0, label %ck1

b0:                                               ; preds = %entry
  tail call void @llvm.experimental.noalias.scope.decl(metadata !4)
  %tr2.i = load volatile i8, ptr @__io_pending, align 1, !noalias !4
  %1 = and i8 %tr2.i, 1
  %pi101.i = icmp ne i8 %1, 0
  tail call void @llvm.assume(i1 %pi101.i)
  %t13.i = add nsw i64 %il7.i2, 1
  store i64 %t13.i, ptr %state, align 8, !alias.scope !4
  %t20.i = srem i64 %t13.i, 100000
  %c26.i = icmp eq i64 %t20.i, 0
  br i1 %c26.i, label %g28_t.i, label %ck1

g28_t.i:                                          ; preds = %b0
  %t29.i = tail call i64 @__print_int(i64 %t13.i), !noalias !4
  br label %ck1

ck1:                                              ; preds = %g28_t.i, %b0, %entry
  ret void
}

; Function Attrs: nofree norecurse nosync nounwind memory(readwrite)
define noundef i32 @main() local_unnamed_addr #5 {
entry:
  tail call void @__rt_init() #7
  tail call void @__rt_poll() #7
  br label %tick

tick:                                             ; preds = %do_wait, %entry
  %tr13 = load volatile i8, ptr @__io_pending, align 1
  br label %uni_work_hdr

uni_work_hdr:                                     ; preds = %uni_work_hdr.backedge, %tick
  %ssa_phi_uni_work.unpack = phi i64 [ 0, %tick ], [ %ssa_phi_uni_work.unpack.be, %uni_work_hdr.backedge ]
  %cpuni_work_103 = icmp slt i64 %ssa_phi_uni_work.unpack, 49999997
  br i1 %cpuni_work_103, label %uni_work_body4, label %uni_work_rem

uni_work_rem:                                     ; preds = %uni_work_hdr
  %cpuni_work_104 = icmp ult i64 %ssa_phi_uni_work.unpack, 50000000
  br i1 %cpuni_work_104, label %uni_work_body1, label %uni_work_done

uni_work_body4:                                   ; preds = %uni_work_hdr
  %t15 = add nsw i64 %ssa_phi_uni_work.unpack, 1
  %t21 = srem i64 %t15, 100000
  %c26 = icmp eq i64 %t21, 0
  br i1 %c26, label %g28_t, label %g28_e

g28_t:                                            ; preds = %uni_work_body4
  %t29 = tail call i64 @__print_int(i64 %t15)
  br label %g28_e

g28_e:                                            ; preds = %g28_t, %uni_work_body4
  %t32 = add nsw i64 %ssa_phi_uni_work.unpack, 2
  %t38 = srem i64 %t32, 100000
  %c43 = icmp eq i64 %t38, 0
  br i1 %c43, label %g45_t, label %g45_e

g45_t:                                            ; preds = %g28_e
  %t46 = tail call i64 @__print_int(i64 %t32)
  br label %g45_e

g45_e:                                            ; preds = %g45_t, %g28_e
  %t49 = add nsw i64 %ssa_phi_uni_work.unpack, 3
  %t55 = srem i64 %t49, 100000
  %c60 = icmp eq i64 %t55, 0
  br i1 %c60, label %g62_t, label %g62_e

g62_t:                                            ; preds = %g45_e
  %t63 = tail call i64 @__print_int(i64 %t49)
  br label %g62_e

g62_e:                                            ; preds = %g62_t, %g45_e
  %t66 = add nsw i64 %ssa_phi_uni_work.unpack, 4
  %t72 = srem i64 %t66, 100000
  %c77 = icmp eq i64 %t72, 0
  br i1 %c77, label %uni_work_hdr.backedge.sink.split, label %uni_work_hdr.backedge

uni_work_hdr.backedge.sink.split:                 ; preds = %g62_e, %uni_work_body1
  %t83.sink = phi i64 [ %t83, %uni_work_body1 ], [ %t66, %g62_e ]
  %t97 = tail call i64 @__print_int(i64 %t83.sink)
  br label %uni_work_hdr.backedge

uni_work_hdr.backedge:                            ; preds = %uni_work_hdr.backedge.sink.split, %uni_work_body1, %g62_e
  %ssa_phi_uni_work.unpack.be = phi i64 [ %t66, %g62_e ], [ %t83, %uni_work_body1 ], [ %t83.sink, %uni_work_hdr.backedge.sink.split ]
  br label %uni_work_hdr

uni_work_body1:                                   ; preds = %uni_work_rem
  %t83 = add nuw nsw i64 %ssa_phi_uni_work.unpack, 1
  %t89.lhs.trunc = trunc i64 %t83 to i32
  %t892 = urem i32 %t89.lhs.trunc, 100000
  %c94 = icmp eq i32 %t892, 0
  br i1 %c94, label %uni_work_hdr.backedge.sink.split, label %uni_work_hdr.backedge

uni_work_done:                                    ; preds = %uni_work_rem
  %t109 = icmp eq i64 %ssa_phi_uni_work.unpack, 50000000
  br i1 %t109, label %done, label %do_wait

do_wait:                                          ; preds = %uni_work_done
  tail call void @__rt_wait() #7
  br label %tick

done:                                             ; preds = %uni_work_done
  ret i32 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: readwrite)
declare void @llvm.experimental.noalias.scope.decl(metadata) #6

attributes #0 = { mustprogress nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: write) }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: write) }
attributes #2 = { alwaysinline mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: write) }
attributes #4 = { mustprogress nofree norecurse nosync nounwind willreturn memory(readwrite) }
attributes #5 = { nofree norecurse nosync nounwind memory(readwrite) }
attributes #6 = { nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: readwrite) }
attributes #7 = { nounwind }

!llvm.wake_triggers = !{!0}

!0 = !{!"__io_pending"}
!1 = !{!2}
!2 = distinct !{!2, !3, !"pre_work: %state"}
!3 = distinct !{!3, !"pre_work"}
!4 = !{!5}
!5 = distinct !{!5, !6, !"work: %state"}
!6 = distinct !{!6, !"work"}
