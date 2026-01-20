; ModuleID = 'xdp-dispatcher.c'
source_filename = "xdp-dispatcher.c"
target datalayout = "e-m:e-p:64:64-i64:64-i128:128-n32:64-S128"
target triple = "bpf"

%struct.xdp_dispatcher_config = type { i8, i8, i8, i8, [10 x i32], [10 x i32], [10 x i32] }

@conf = internal constant %struct.xdp_dispatcher_config zeroinitializer, align 4, !dbg !0
@_license = dso_local global [4 x i8] c"GPL\00", section "license", align 1, !dbg !15
@dispatcher_version = dso_local global ptr null, section "xdp_metadata", align 8, !dbg !21
@llvm.compiler.used = appending global [4 x ptr] [ptr @_license, ptr @dispatcher_version, ptr @xdp_dispatcher, ptr @xdp_pass], section "llvm.metadata"

; Function Attrs: nofree noinline norecurse nounwind memory(inaccessiblemem: readwrite)
define dso_local i32 @prog0(ptr noundef readnone captures(address_is_null) %0) local_unnamed_addr #0 !dbg !53 {
  %2 = alloca i32, align 4, !DIAssignID !69
    #dbg_assign(i1 poison, !67, !DIExpression(), !69, ptr %2, !DIExpression(), !70)
    #dbg_value(ptr %0, !66, !DIExpression(), !70)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %2), !dbg !71
  store volatile i32 31, ptr %2, align 4, !dbg !72, !tbaa !73, !DIAssignID !77
    #dbg_assign(i32 31, !67, !DIExpression(), !77, ptr %2, !DIExpression(), !70)
  %3 = icmp eq ptr %0, null, !dbg !78
  br i1 %3, label %6, label %4, !dbg !80

4:                                                ; preds = %1
  %5 = load volatile i32, ptr %2, align 4, !dbg !81, !tbaa !73
  br label %6, !dbg !82

6:                                                ; preds = %1, %4
  %7 = phi i32 [ %5, %4 ], [ 0, %1 ], !dbg !70
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %2), !dbg !83
  ret i32 %7, !dbg !83
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(i64 immarg, ptr captures(none)) #1

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(i64 immarg, ptr captures(none)) #1

; Function Attrs: nofree noinline norecurse nounwind memory(inaccessiblemem: readwrite)
define dso_local i32 @prog1(ptr noundef readnone captures(address_is_null) %0) local_unnamed_addr #0 !dbg !84 {
  %2 = alloca i32, align 4, !DIAssignID !88
    #dbg_assign(i1 poison, !87, !DIExpression(), !88, ptr %2, !DIExpression(), !89)
    #dbg_value(ptr %0, !86, !DIExpression(), !89)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %2), !dbg !90
  store volatile i32 31, ptr %2, align 4, !dbg !91, !tbaa !73, !DIAssignID !92
    #dbg_assign(i32 31, !87, !DIExpression(), !92, ptr %2, !DIExpression(), !89)
  %3 = icmp eq ptr %0, null, !dbg !93
  br i1 %3, label %6, label %4, !dbg !95

4:                                                ; preds = %1
  %5 = load volatile i32, ptr %2, align 4, !dbg !96, !tbaa !73
  br label %6, !dbg !97

6:                                                ; preds = %1, %4
  %7 = phi i32 [ %5, %4 ], [ 0, %1 ], !dbg !89
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %2), !dbg !98
  ret i32 %7, !dbg !98
}

; Function Attrs: nofree noinline norecurse nounwind memory(inaccessiblemem: readwrite)
define dso_local i32 @prog2(ptr noundef readnone captures(address_is_null) %0) local_unnamed_addr #0 !dbg !99 {
  %2 = alloca i32, align 4, !DIAssignID !103
    #dbg_assign(i1 poison, !102, !DIExpression(), !103, ptr %2, !DIExpression(), !104)
    #dbg_value(ptr %0, !101, !DIExpression(), !104)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %2), !dbg !105
  store volatile i32 31, ptr %2, align 4, !dbg !106, !tbaa !73, !DIAssignID !107
    #dbg_assign(i32 31, !102, !DIExpression(), !107, ptr %2, !DIExpression(), !104)
  %3 = icmp eq ptr %0, null, !dbg !108
  br i1 %3, label %6, label %4, !dbg !110

4:                                                ; preds = %1
  %5 = load volatile i32, ptr %2, align 4, !dbg !111, !tbaa !73
  br label %6, !dbg !112

6:                                                ; preds = %1, %4
  %7 = phi i32 [ %5, %4 ], [ 0, %1 ], !dbg !104
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %2), !dbg !113
  ret i32 %7, !dbg !113
}

; Function Attrs: nofree noinline norecurse nounwind memory(inaccessiblemem: readwrite)
define dso_local i32 @prog3(ptr noundef readnone captures(address_is_null) %0) local_unnamed_addr #0 !dbg !114 {
  %2 = alloca i32, align 4, !DIAssignID !118
    #dbg_assign(i1 poison, !117, !DIExpression(), !118, ptr %2, !DIExpression(), !119)
    #dbg_value(ptr %0, !116, !DIExpression(), !119)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %2), !dbg !120
  store volatile i32 31, ptr %2, align 4, !dbg !121, !tbaa !73, !DIAssignID !122
    #dbg_assign(i32 31, !117, !DIExpression(), !122, ptr %2, !DIExpression(), !119)
  %3 = icmp eq ptr %0, null, !dbg !123
  br i1 %3, label %6, label %4, !dbg !125

4:                                                ; preds = %1
  %5 = load volatile i32, ptr %2, align 4, !dbg !126, !tbaa !73
  br label %6, !dbg !127

6:                                                ; preds = %1, %4
  %7 = phi i32 [ %5, %4 ], [ 0, %1 ], !dbg !119
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %2), !dbg !128
  ret i32 %7, !dbg !128
}

; Function Attrs: nofree noinline norecurse nounwind memory(inaccessiblemem: readwrite)
define dso_local i32 @prog4(ptr noundef readnone captures(address_is_null) %0) local_unnamed_addr #0 !dbg !129 {
  %2 = alloca i32, align 4, !DIAssignID !133
    #dbg_assign(i1 poison, !132, !DIExpression(), !133, ptr %2, !DIExpression(), !134)
    #dbg_value(ptr %0, !131, !DIExpression(), !134)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %2), !dbg !135
  store volatile i32 31, ptr %2, align 4, !dbg !136, !tbaa !73, !DIAssignID !137
    #dbg_assign(i32 31, !132, !DIExpression(), !137, ptr %2, !DIExpression(), !134)
  %3 = icmp eq ptr %0, null, !dbg !138
  br i1 %3, label %6, label %4, !dbg !140

4:                                                ; preds = %1
  %5 = load volatile i32, ptr %2, align 4, !dbg !141, !tbaa !73
  br label %6, !dbg !142

6:                                                ; preds = %1, %4
  %7 = phi i32 [ %5, %4 ], [ 0, %1 ], !dbg !134
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %2), !dbg !143
  ret i32 %7, !dbg !143
}

; Function Attrs: nofree noinline norecurse nounwind memory(inaccessiblemem: readwrite)
define dso_local i32 @prog5(ptr noundef readnone captures(address_is_null) %0) local_unnamed_addr #0 !dbg !144 {
  %2 = alloca i32, align 4, !DIAssignID !148
    #dbg_assign(i1 poison, !147, !DIExpression(), !148, ptr %2, !DIExpression(), !149)
    #dbg_value(ptr %0, !146, !DIExpression(), !149)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %2), !dbg !150
  store volatile i32 31, ptr %2, align 4, !dbg !151, !tbaa !73, !DIAssignID !152
    #dbg_assign(i32 31, !147, !DIExpression(), !152, ptr %2, !DIExpression(), !149)
  %3 = icmp eq ptr %0, null, !dbg !153
  br i1 %3, label %6, label %4, !dbg !155

4:                                                ; preds = %1
  %5 = load volatile i32, ptr %2, align 4, !dbg !156, !tbaa !73
  br label %6, !dbg !157

6:                                                ; preds = %1, %4
  %7 = phi i32 [ %5, %4 ], [ 0, %1 ], !dbg !149
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %2), !dbg !158
  ret i32 %7, !dbg !158
}

; Function Attrs: nofree noinline norecurse nounwind memory(inaccessiblemem: readwrite)
define dso_local i32 @prog6(ptr noundef readnone captures(address_is_null) %0) local_unnamed_addr #0 !dbg !159 {
  %2 = alloca i32, align 4, !DIAssignID !163
    #dbg_assign(i1 poison, !162, !DIExpression(), !163, ptr %2, !DIExpression(), !164)
    #dbg_value(ptr %0, !161, !DIExpression(), !164)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %2), !dbg !165
  store volatile i32 31, ptr %2, align 4, !dbg !166, !tbaa !73, !DIAssignID !167
    #dbg_assign(i32 31, !162, !DIExpression(), !167, ptr %2, !DIExpression(), !164)
  %3 = icmp eq ptr %0, null, !dbg !168
  br i1 %3, label %6, label %4, !dbg !170

4:                                                ; preds = %1
  %5 = load volatile i32, ptr %2, align 4, !dbg !171, !tbaa !73
  br label %6, !dbg !172

6:                                                ; preds = %1, %4
  %7 = phi i32 [ %5, %4 ], [ 0, %1 ], !dbg !164
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %2), !dbg !173
  ret i32 %7, !dbg !173
}

; Function Attrs: nofree noinline norecurse nounwind memory(inaccessiblemem: readwrite)
define dso_local i32 @prog7(ptr noundef readnone captures(address_is_null) %0) local_unnamed_addr #0 !dbg !174 {
  %2 = alloca i32, align 4, !DIAssignID !178
    #dbg_assign(i1 poison, !177, !DIExpression(), !178, ptr %2, !DIExpression(), !179)
    #dbg_value(ptr %0, !176, !DIExpression(), !179)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %2), !dbg !180
  store volatile i32 31, ptr %2, align 4, !dbg !181, !tbaa !73, !DIAssignID !182
    #dbg_assign(i32 31, !177, !DIExpression(), !182, ptr %2, !DIExpression(), !179)
  %3 = icmp eq ptr %0, null, !dbg !183
  br i1 %3, label %6, label %4, !dbg !185

4:                                                ; preds = %1
  %5 = load volatile i32, ptr %2, align 4, !dbg !186, !tbaa !73
  br label %6, !dbg !187

6:                                                ; preds = %1, %4
  %7 = phi i32 [ %5, %4 ], [ 0, %1 ], !dbg !179
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %2), !dbg !188
  ret i32 %7, !dbg !188
}

; Function Attrs: nofree noinline norecurse nounwind memory(inaccessiblemem: readwrite)
define dso_local i32 @prog8(ptr noundef readnone captures(address_is_null) %0) local_unnamed_addr #0 !dbg !189 {
  %2 = alloca i32, align 4, !DIAssignID !193
    #dbg_assign(i1 poison, !192, !DIExpression(), !193, ptr %2, !DIExpression(), !194)
    #dbg_value(ptr %0, !191, !DIExpression(), !194)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %2), !dbg !195
  store volatile i32 31, ptr %2, align 4, !dbg !196, !tbaa !73, !DIAssignID !197
    #dbg_assign(i32 31, !192, !DIExpression(), !197, ptr %2, !DIExpression(), !194)
  %3 = icmp eq ptr %0, null, !dbg !198
  br i1 %3, label %6, label %4, !dbg !200

4:                                                ; preds = %1
  %5 = load volatile i32, ptr %2, align 4, !dbg !201, !tbaa !73
  br label %6, !dbg !202

6:                                                ; preds = %1, %4
  %7 = phi i32 [ %5, %4 ], [ 0, %1 ], !dbg !194
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %2), !dbg !203
  ret i32 %7, !dbg !203
}

; Function Attrs: nofree noinline norecurse nounwind memory(inaccessiblemem: readwrite)
define dso_local i32 @prog9(ptr noundef readnone captures(address_is_null) %0) local_unnamed_addr #0 !dbg !204 {
  %2 = alloca i32, align 4, !DIAssignID !208
    #dbg_assign(i1 poison, !207, !DIExpression(), !208, ptr %2, !DIExpression(), !209)
    #dbg_value(ptr %0, !206, !DIExpression(), !209)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %2), !dbg !210
  store volatile i32 31, ptr %2, align 4, !dbg !211, !tbaa !73, !DIAssignID !212
    #dbg_assign(i32 31, !207, !DIExpression(), !212, ptr %2, !DIExpression(), !209)
  %3 = icmp eq ptr %0, null, !dbg !213
  br i1 %3, label %6, label %4, !dbg !215

4:                                                ; preds = %1
  %5 = load volatile i32, ptr %2, align 4, !dbg !216, !tbaa !73
  br label %6, !dbg !217

6:                                                ; preds = %1, %4
  %7 = phi i32 [ %5, %4 ], [ 0, %1 ], !dbg !209
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %2), !dbg !218
  ret i32 %7, !dbg !218
}

; Function Attrs: nofree noinline norecurse nounwind memory(inaccessiblemem: readwrite)
define dso_local i32 @compat_test(ptr noundef readnone captures(address_is_null) %0) local_unnamed_addr #0 !dbg !219 {
  %2 = alloca i32, align 4, !DIAssignID !223
    #dbg_assign(i1 poison, !222, !DIExpression(), !223, ptr %2, !DIExpression(), !224)
    #dbg_value(ptr %0, !221, !DIExpression(), !224)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %2), !dbg !225
  store volatile i32 31, ptr %2, align 4, !dbg !226, !tbaa !73, !DIAssignID !227
    #dbg_assign(i32 31, !222, !DIExpression(), !227, ptr %2, !DIExpression(), !224)
  %3 = icmp eq ptr %0, null, !dbg !228
  br i1 %3, label %6, label %4, !dbg !230

4:                                                ; preds = %1
  %5 = load volatile i32, ptr %2, align 4, !dbg !231, !tbaa !73
  br label %6, !dbg !232

6:                                                ; preds = %1, %4
  %7 = phi i32 [ %5, %4 ], [ 0, %1 ], !dbg !224
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %2), !dbg !233
  ret i32 %7, !dbg !233
}

; Function Attrs: nofree norecurse nounwind memory(inaccessiblemem: readwrite)
define dso_local i32 @xdp_dispatcher(ptr noundef readnone captures(address_is_null) %0) #2 section "xdp" !dbg !234 {
    #dbg_value(ptr %0, !236, !DIExpression(), !240)
  %2 = load volatile i8, ptr getelementptr inbounds nuw (i8, ptr @conf, i64 2), align 2, !dbg !241, !tbaa !242
    #dbg_value(i8 %2, !237, !DIExpression(), !240)
  %3 = icmp eq i8 %2, 0, !dbg !244
  br i1 %3, label %86, label %4, !dbg !244

4:                                                ; preds = %1
  %5 = tail call i32 @prog0(ptr noundef %0), !dbg !246
    #dbg_value(i32 %5, !238, !DIExpression(), !240)
  %6 = shl nuw i32 1, %5, !dbg !247
  %7 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @conf, i64 4), align 4, !dbg !249, !tbaa !73
  %8 = and i32 %6, %7, !dbg !250
  %9 = icmp eq i32 %8, 0, !dbg !250
  br i1 %9, label %86, label %10, !dbg !251

10:                                               ; preds = %4
  %11 = icmp eq i8 %2, 1, !dbg !252
  br i1 %11, label %86, label %12, !dbg !252

12:                                               ; preds = %10
  %13 = tail call i32 @prog1(ptr noundef %0), !dbg !254
    #dbg_value(i32 %13, !238, !DIExpression(), !240)
  %14 = shl nuw i32 1, %13, !dbg !255
  %15 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @conf, i64 8), align 4, !dbg !257, !tbaa !73
  %16 = and i32 %14, %15, !dbg !258
  %17 = icmp eq i32 %16, 0, !dbg !258
  br i1 %17, label %86, label %18, !dbg !259

18:                                               ; preds = %12
  %19 = icmp ult i8 %2, 3, !dbg !260
  br i1 %19, label %86, label %20, !dbg !260

20:                                               ; preds = %18
  %21 = tail call i32 @prog2(ptr noundef %0), !dbg !262
    #dbg_value(i32 %21, !238, !DIExpression(), !240)
  %22 = shl nuw i32 1, %21, !dbg !263
  %23 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @conf, i64 12), align 4, !dbg !265, !tbaa !73
  %24 = and i32 %22, %23, !dbg !266
  %25 = icmp eq i32 %24, 0, !dbg !266
  br i1 %25, label %86, label %26, !dbg !267

26:                                               ; preds = %20
  %27 = icmp eq i8 %2, 3, !dbg !268
  br i1 %27, label %86, label %28, !dbg !268

28:                                               ; preds = %26
  %29 = tail call i32 @prog3(ptr noundef %0), !dbg !270
    #dbg_value(i32 %29, !238, !DIExpression(), !240)
  %30 = shl nuw i32 1, %29, !dbg !271
  %31 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @conf, i64 16), align 4, !dbg !273, !tbaa !73
  %32 = and i32 %30, %31, !dbg !274
  %33 = icmp eq i32 %32, 0, !dbg !274
  br i1 %33, label %86, label %34, !dbg !275

34:                                               ; preds = %28
  %35 = icmp ult i8 %2, 5, !dbg !276
  br i1 %35, label %86, label %36, !dbg !276

36:                                               ; preds = %34
  %37 = tail call i32 @prog4(ptr noundef %0), !dbg !278
    #dbg_value(i32 %37, !238, !DIExpression(), !240)
  %38 = shl nuw i32 1, %37, !dbg !279
  %39 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @conf, i64 20), align 4, !dbg !281, !tbaa !73
  %40 = and i32 %38, %39, !dbg !282
  %41 = icmp eq i32 %40, 0, !dbg !282
  br i1 %41, label %86, label %42, !dbg !283

42:                                               ; preds = %36
  %43 = icmp eq i8 %2, 5, !dbg !284
  br i1 %43, label %86, label %44, !dbg !284

44:                                               ; preds = %42
  %45 = tail call i32 @prog5(ptr noundef %0), !dbg !286
    #dbg_value(i32 %45, !238, !DIExpression(), !240)
  %46 = shl nuw i32 1, %45, !dbg !287
  %47 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @conf, i64 24), align 4, !dbg !289, !tbaa !73
  %48 = and i32 %46, %47, !dbg !290
  %49 = icmp eq i32 %48, 0, !dbg !290
  br i1 %49, label %86, label %50, !dbg !291

50:                                               ; preds = %44
  %51 = icmp ult i8 %2, 7, !dbg !292
  br i1 %51, label %86, label %52, !dbg !292

52:                                               ; preds = %50
  %53 = tail call i32 @prog6(ptr noundef %0), !dbg !294
    #dbg_value(i32 %53, !238, !DIExpression(), !240)
  %54 = shl nuw i32 1, %53, !dbg !295
  %55 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @conf, i64 28), align 4, !dbg !297, !tbaa !73
  %56 = and i32 %54, %55, !dbg !298
  %57 = icmp eq i32 %56, 0, !dbg !298
  br i1 %57, label %86, label %58, !dbg !299

58:                                               ; preds = %52
  %59 = icmp eq i8 %2, 7, !dbg !300
  br i1 %59, label %86, label %60, !dbg !300

60:                                               ; preds = %58
  %61 = tail call i32 @prog7(ptr noundef %0), !dbg !302
    #dbg_value(i32 %61, !238, !DIExpression(), !240)
  %62 = shl nuw i32 1, %61, !dbg !303
  %63 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @conf, i64 32), align 4, !dbg !305, !tbaa !73
  %64 = and i32 %62, %63, !dbg !306
  %65 = icmp eq i32 %64, 0, !dbg !306
  br i1 %65, label %86, label %66, !dbg !307

66:                                               ; preds = %60
  %67 = icmp ult i8 %2, 9, !dbg !308
  br i1 %67, label %86, label %68, !dbg !308

68:                                               ; preds = %66
  %69 = tail call i32 @prog8(ptr noundef %0), !dbg !310
    #dbg_value(i32 %69, !238, !DIExpression(), !240)
  %70 = shl nuw i32 1, %69, !dbg !311
  %71 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @conf, i64 36), align 4, !dbg !313, !tbaa !73
  %72 = and i32 %70, %71, !dbg !314
  %73 = icmp eq i32 %72, 0, !dbg !314
  br i1 %73, label %86, label %74, !dbg !315

74:                                               ; preds = %68
  %75 = icmp eq i8 %2, 9, !dbg !316
  br i1 %75, label %86, label %76, !dbg !316

76:                                               ; preds = %74
  %77 = tail call i32 @prog9(ptr noundef %0), !dbg !318
    #dbg_value(i32 %77, !238, !DIExpression(), !240)
  %78 = shl nuw i32 1, %77, !dbg !319
  %79 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @conf, i64 40), align 4, !dbg !321, !tbaa !73
  %80 = and i32 %78, %79, !dbg !322
  %81 = icmp eq i32 %80, 0, !dbg !322
  br i1 %81, label %86, label %82, !dbg !323

82:                                               ; preds = %76
  %83 = icmp ult i8 %2, 11, !dbg !324
  br i1 %83, label %86, label %84, !dbg !324

84:                                               ; preds = %82
  %85 = tail call i32 @compat_test(ptr noundef %0), !dbg !326
    #dbg_value(i32 %85, !238, !DIExpression(), !240)
  br label %86, !dbg !327

86:                                               ; preds = %84, %1, %10, %18, %26, %34, %42, %50, %58, %66, %74, %82, %76, %68, %60, %52, %44, %36, %28, %20, %12, %4
  %87 = phi i32 [ %5, %4 ], [ %13, %12 ], [ %21, %20 ], [ %29, %28 ], [ %37, %36 ], [ %45, %44 ], [ %53, %52 ], [ %61, %60 ], [ %69, %68 ], [ %77, %76 ], [ 2, %82 ], [ 2, %74 ], [ 2, %66 ], [ 2, %58 ], [ 2, %50 ], [ 2, %42 ], [ 2, %34 ], [ 2, %26 ], [ 2, %18 ], [ 2, %10 ], [ 2, %1 ], [ 2, %84 ], !dbg !240
  ret i32 %87, !dbg !328
}

; Function Attrs: mustprogress nofree norecurse nosync nounwind willreturn memory(none)
define dso_local noundef i32 @xdp_pass(ptr readnone captures(none) %0) #3 section "xdp" !dbg !329 {
    #dbg_value(ptr poison, !331, !DIExpression(), !332)
  ret i32 2, !dbg !333
}

attributes #0 = { nofree noinline norecurse nounwind memory(inaccessiblemem: readwrite) "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nofree norecurse nounwind memory(inaccessiblemem: readwrite) "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { mustprogress nofree norecurse nosync nounwind willreturn memory(none) "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

!llvm.dbg.cu = !{!2}
!llvm.module.flags = !{!47, !48, !49, !50, !51}
!llvm.ident = !{!52}

!0 = !DIGlobalVariableExpression(var: !1, expr: !DIExpression())
!1 = distinct !DIGlobalVariable(name: "conf", scope: !2, file: !3, line: 20, type: !28, isLocal: true, isDefinition: true)
!2 = distinct !DICompileUnit(language: DW_LANG_C11, file: !3, producer: "clang version 21.1.7 (Fedora 21.1.7-1.fc43)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, enums: !4, globals: !14, splitDebugInlining: false, nameTableKind: None)
!3 = !DIFile(filename: "xdp-dispatcher.c", directory: "/var/home/leonardogiovannoni/Src/Net/original/wowow3223o/libxdp-sys/xdp-tools/lib/libxdp", checksumkind: CSK_MD5, checksum: "b6ed117e39c1c72628611da72a4d5f5d")
!4 = !{!5}
!5 = !DICompositeType(tag: DW_TAG_enumeration_type, name: "xdp_action", file: !6, line: 5933, baseType: !7, size: 32, elements: !8)
!6 = !DIFile(filename: "../../headers/linux/bpf.h", directory: "/var/home/leonardogiovannoni/Src/Net/original/wowow3223o/libxdp-sys/xdp-tools/lib/libxdp", checksumkind: CSK_MD5, checksum: "19e7a278dd5e69adb087c419977e86e0")
!7 = !DIBasicType(name: "unsigned int", size: 32, encoding: DW_ATE_unsigned)
!8 = !{!9, !10, !11, !12, !13}
!9 = !DIEnumerator(name: "XDP_ABORTED", value: 0)
!10 = !DIEnumerator(name: "XDP_DROP", value: 1)
!11 = !DIEnumerator(name: "XDP_PASS", value: 2)
!12 = !DIEnumerator(name: "XDP_TX", value: 3)
!13 = !DIEnumerator(name: "XDP_REDIRECT", value: 4)
!14 = !{!15, !21, !0}
!15 = !DIGlobalVariableExpression(var: !16, expr: !DIExpression())
!16 = distinct !DIGlobalVariable(name: "_license", scope: !2, file: !3, line: 199, type: !17, isLocal: false, isDefinition: true)
!17 = !DICompositeType(tag: DW_TAG_array_type, baseType: !18, size: 32, elements: !19)
!18 = !DIBasicType(name: "char", size: 8, encoding: DW_ATE_signed_char)
!19 = !{!20}
!20 = !DISubrange(count: 4)
!21 = !DIGlobalVariableExpression(var: !22, expr: !DIExpression())
!22 = distinct !DIGlobalVariable(name: "dispatcher_version", scope: !2, file: !3, line: 200, type: !23, isLocal: false, isDefinition: true)
!23 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !24, size: 64)
!24 = !DICompositeType(tag: DW_TAG_array_type, baseType: !25, size: 64, elements: !26)
!25 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!26 = !{!27}
!27 = !DISubrange(count: 2)
!28 = !DIDerivedType(tag: DW_TAG_const_type, baseType: !29)
!29 = !DIDerivedType(tag: DW_TAG_volatile_type, baseType: !30)
!30 = distinct !DICompositeType(tag: DW_TAG_structure_type, name: "xdp_dispatcher_config", file: !31, line: 24, size: 992, elements: !32)
!31 = !DIFile(filename: "../../headers/xdp/prog_dispatcher.h", directory: "/var/home/leonardogiovannoni/Src/Net/original/wowow3223o/libxdp-sys/xdp-tools/lib/libxdp", checksumkind: CSK_MD5, checksum: "aaec112d5bc5c2d2bb62c79d8f95df8a")
!32 = !{!33, !37, !38, !39, !40, !45, !46}
!33 = !DIDerivedType(tag: DW_TAG_member, name: "magic", scope: !30, file: !31, line: 25, baseType: !34, size: 8)
!34 = !DIDerivedType(tag: DW_TAG_typedef, name: "__u8", file: !35, line: 21, baseType: !36)
!35 = !DIFile(filename: "/usr/include/asm-generic/int-ll64.h", directory: "", checksumkind: CSK_MD5, checksum: "b810f270733e106319b67ef512c6246e")
!36 = !DIBasicType(name: "unsigned char", size: 8, encoding: DW_ATE_unsigned_char)
!37 = !DIDerivedType(tag: DW_TAG_member, name: "dispatcher_version", scope: !30, file: !31, line: 26, baseType: !34, size: 8, offset: 8)
!38 = !DIDerivedType(tag: DW_TAG_member, name: "num_progs_enabled", scope: !30, file: !31, line: 27, baseType: !34, size: 8, offset: 16)
!39 = !DIDerivedType(tag: DW_TAG_member, name: "is_xdp_frags", scope: !30, file: !31, line: 28, baseType: !34, size: 8, offset: 24)
!40 = !DIDerivedType(tag: DW_TAG_member, name: "chain_call_actions", scope: !30, file: !31, line: 29, baseType: !41, size: 320, offset: 32)
!41 = !DICompositeType(tag: DW_TAG_array_type, baseType: !42, size: 320, elements: !43)
!42 = !DIDerivedType(tag: DW_TAG_typedef, name: "__u32", file: !35, line: 27, baseType: !7)
!43 = !{!44}
!44 = !DISubrange(count: 10)
!45 = !DIDerivedType(tag: DW_TAG_member, name: "run_prios", scope: !30, file: !31, line: 30, baseType: !41, size: 320, offset: 352)
!46 = !DIDerivedType(tag: DW_TAG_member, name: "program_flags", scope: !30, file: !31, line: 31, baseType: !41, size: 320, offset: 672)
!47 = !{i32 7, !"Dwarf Version", i32 5}
!48 = !{i32 2, !"Debug Info Version", i32 3}
!49 = !{i32 1, !"wchar_size", i32 4}
!50 = !{i32 7, !"frame-pointer", i32 2}
!51 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!52 = !{!"clang version 21.1.7 (Fedora 21.1.7-1.fc43)"}
!53 = distinct !DISubprogram(name: "prog0", scope: !3, file: !3, line: 26, type: !54, scopeLine: 26, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !65)
!54 = !DISubroutineType(types: !55)
!55 = !{!25, !56}
!56 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !57, size: 64)
!57 = distinct !DICompositeType(tag: DW_TAG_structure_type, name: "xdp_md", file: !6, line: 5944, size: 192, elements: !58)
!58 = !{!59, !60, !61, !62, !63, !64}
!59 = !DIDerivedType(tag: DW_TAG_member, name: "data", scope: !57, file: !6, line: 5945, baseType: !42, size: 32)
!60 = !DIDerivedType(tag: DW_TAG_member, name: "data_end", scope: !57, file: !6, line: 5946, baseType: !42, size: 32, offset: 32)
!61 = !DIDerivedType(tag: DW_TAG_member, name: "data_meta", scope: !57, file: !6, line: 5947, baseType: !42, size: 32, offset: 64)
!62 = !DIDerivedType(tag: DW_TAG_member, name: "ingress_ifindex", scope: !57, file: !6, line: 5949, baseType: !42, size: 32, offset: 96)
!63 = !DIDerivedType(tag: DW_TAG_member, name: "rx_queue_index", scope: !57, file: !6, line: 5950, baseType: !42, size: 32, offset: 128)
!64 = !DIDerivedType(tag: DW_TAG_member, name: "egress_ifindex", scope: !57, file: !6, line: 5952, baseType: !42, size: 32, offset: 160)
!65 = !{!66, !67}
!66 = !DILocalVariable(name: "ctx", arg: 1, scope: !53, file: !3, line: 26, type: !56)
!67 = !DILocalVariable(name: "ret", scope: !53, file: !3, line: 27, type: !68)
!68 = !DIDerivedType(tag: DW_TAG_volatile_type, baseType: !25)
!69 = distinct !DIAssignID()
!70 = !DILocation(line: 0, scope: !53)
!71 = !DILocation(line: 27, column: 9, scope: !53)
!72 = !DILocation(line: 27, column: 22, scope: !53)
!73 = !{!74, !74, i64 0}
!74 = !{!"int", !75, i64 0}
!75 = !{!"omnipotent char", !76, i64 0}
!76 = !{!"Simple C/C++ TBAA"}
!77 = distinct !DIAssignID()
!78 = !DILocation(line: 29, column: 14, scope: !79)
!79 = distinct !DILexicalBlock(scope: !53, file: !3, line: 29, column: 13)
!80 = !DILocation(line: 29, column: 13, scope: !79)
!81 = !DILocation(line: 31, column: 16, scope: !53)
!82 = !DILocation(line: 31, column: 9, scope: !53)
!83 = !DILocation(line: 32, column: 1, scope: !53)
!84 = distinct !DISubprogram(name: "prog1", scope: !3, file: !3, line: 34, type: !54, scopeLine: 34, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !85)
!85 = !{!86, !87}
!86 = !DILocalVariable(name: "ctx", arg: 1, scope: !84, file: !3, line: 34, type: !56)
!87 = !DILocalVariable(name: "ret", scope: !84, file: !3, line: 35, type: !68)
!88 = distinct !DIAssignID()
!89 = !DILocation(line: 0, scope: !84)
!90 = !DILocation(line: 35, column: 9, scope: !84)
!91 = !DILocation(line: 35, column: 22, scope: !84)
!92 = distinct !DIAssignID()
!93 = !DILocation(line: 37, column: 14, scope: !94)
!94 = distinct !DILexicalBlock(scope: !84, file: !3, line: 37, column: 13)
!95 = !DILocation(line: 37, column: 13, scope: !94)
!96 = !DILocation(line: 39, column: 16, scope: !84)
!97 = !DILocation(line: 39, column: 9, scope: !84)
!98 = !DILocation(line: 40, column: 1, scope: !84)
!99 = distinct !DISubprogram(name: "prog2", scope: !3, file: !3, line: 42, type: !54, scopeLine: 42, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !100)
!100 = !{!101, !102}
!101 = !DILocalVariable(name: "ctx", arg: 1, scope: !99, file: !3, line: 42, type: !56)
!102 = !DILocalVariable(name: "ret", scope: !99, file: !3, line: 43, type: !68)
!103 = distinct !DIAssignID()
!104 = !DILocation(line: 0, scope: !99)
!105 = !DILocation(line: 43, column: 9, scope: !99)
!106 = !DILocation(line: 43, column: 22, scope: !99)
!107 = distinct !DIAssignID()
!108 = !DILocation(line: 45, column: 14, scope: !109)
!109 = distinct !DILexicalBlock(scope: !99, file: !3, line: 45, column: 13)
!110 = !DILocation(line: 45, column: 13, scope: !109)
!111 = !DILocation(line: 47, column: 16, scope: !99)
!112 = !DILocation(line: 47, column: 9, scope: !99)
!113 = !DILocation(line: 48, column: 1, scope: !99)
!114 = distinct !DISubprogram(name: "prog3", scope: !3, file: !3, line: 50, type: !54, scopeLine: 50, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !115)
!115 = !{!116, !117}
!116 = !DILocalVariable(name: "ctx", arg: 1, scope: !114, file: !3, line: 50, type: !56)
!117 = !DILocalVariable(name: "ret", scope: !114, file: !3, line: 51, type: !68)
!118 = distinct !DIAssignID()
!119 = !DILocation(line: 0, scope: !114)
!120 = !DILocation(line: 51, column: 9, scope: !114)
!121 = !DILocation(line: 51, column: 22, scope: !114)
!122 = distinct !DIAssignID()
!123 = !DILocation(line: 53, column: 14, scope: !124)
!124 = distinct !DILexicalBlock(scope: !114, file: !3, line: 53, column: 13)
!125 = !DILocation(line: 53, column: 13, scope: !124)
!126 = !DILocation(line: 55, column: 16, scope: !114)
!127 = !DILocation(line: 55, column: 9, scope: !114)
!128 = !DILocation(line: 56, column: 1, scope: !114)
!129 = distinct !DISubprogram(name: "prog4", scope: !3, file: !3, line: 58, type: !54, scopeLine: 58, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !130)
!130 = !{!131, !132}
!131 = !DILocalVariable(name: "ctx", arg: 1, scope: !129, file: !3, line: 58, type: !56)
!132 = !DILocalVariable(name: "ret", scope: !129, file: !3, line: 59, type: !68)
!133 = distinct !DIAssignID()
!134 = !DILocation(line: 0, scope: !129)
!135 = !DILocation(line: 59, column: 9, scope: !129)
!136 = !DILocation(line: 59, column: 22, scope: !129)
!137 = distinct !DIAssignID()
!138 = !DILocation(line: 61, column: 14, scope: !139)
!139 = distinct !DILexicalBlock(scope: !129, file: !3, line: 61, column: 13)
!140 = !DILocation(line: 61, column: 13, scope: !139)
!141 = !DILocation(line: 63, column: 16, scope: !129)
!142 = !DILocation(line: 63, column: 9, scope: !129)
!143 = !DILocation(line: 64, column: 1, scope: !129)
!144 = distinct !DISubprogram(name: "prog5", scope: !3, file: !3, line: 66, type: !54, scopeLine: 66, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !145)
!145 = !{!146, !147}
!146 = !DILocalVariable(name: "ctx", arg: 1, scope: !144, file: !3, line: 66, type: !56)
!147 = !DILocalVariable(name: "ret", scope: !144, file: !3, line: 67, type: !68)
!148 = distinct !DIAssignID()
!149 = !DILocation(line: 0, scope: !144)
!150 = !DILocation(line: 67, column: 9, scope: !144)
!151 = !DILocation(line: 67, column: 22, scope: !144)
!152 = distinct !DIAssignID()
!153 = !DILocation(line: 69, column: 14, scope: !154)
!154 = distinct !DILexicalBlock(scope: !144, file: !3, line: 69, column: 13)
!155 = !DILocation(line: 69, column: 13, scope: !154)
!156 = !DILocation(line: 71, column: 16, scope: !144)
!157 = !DILocation(line: 71, column: 9, scope: !144)
!158 = !DILocation(line: 72, column: 1, scope: !144)
!159 = distinct !DISubprogram(name: "prog6", scope: !3, file: !3, line: 74, type: !54, scopeLine: 74, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !160)
!160 = !{!161, !162}
!161 = !DILocalVariable(name: "ctx", arg: 1, scope: !159, file: !3, line: 74, type: !56)
!162 = !DILocalVariable(name: "ret", scope: !159, file: !3, line: 75, type: !68)
!163 = distinct !DIAssignID()
!164 = !DILocation(line: 0, scope: !159)
!165 = !DILocation(line: 75, column: 9, scope: !159)
!166 = !DILocation(line: 75, column: 22, scope: !159)
!167 = distinct !DIAssignID()
!168 = !DILocation(line: 77, column: 14, scope: !169)
!169 = distinct !DILexicalBlock(scope: !159, file: !3, line: 77, column: 13)
!170 = !DILocation(line: 77, column: 13, scope: !169)
!171 = !DILocation(line: 79, column: 16, scope: !159)
!172 = !DILocation(line: 79, column: 9, scope: !159)
!173 = !DILocation(line: 80, column: 1, scope: !159)
!174 = distinct !DISubprogram(name: "prog7", scope: !3, file: !3, line: 82, type: !54, scopeLine: 82, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !175)
!175 = !{!176, !177}
!176 = !DILocalVariable(name: "ctx", arg: 1, scope: !174, file: !3, line: 82, type: !56)
!177 = !DILocalVariable(name: "ret", scope: !174, file: !3, line: 83, type: !68)
!178 = distinct !DIAssignID()
!179 = !DILocation(line: 0, scope: !174)
!180 = !DILocation(line: 83, column: 9, scope: !174)
!181 = !DILocation(line: 83, column: 22, scope: !174)
!182 = distinct !DIAssignID()
!183 = !DILocation(line: 85, column: 14, scope: !184)
!184 = distinct !DILexicalBlock(scope: !174, file: !3, line: 85, column: 13)
!185 = !DILocation(line: 85, column: 13, scope: !184)
!186 = !DILocation(line: 87, column: 16, scope: !174)
!187 = !DILocation(line: 87, column: 9, scope: !174)
!188 = !DILocation(line: 88, column: 1, scope: !174)
!189 = distinct !DISubprogram(name: "prog8", scope: !3, file: !3, line: 90, type: !54, scopeLine: 90, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !190)
!190 = !{!191, !192}
!191 = !DILocalVariable(name: "ctx", arg: 1, scope: !189, file: !3, line: 90, type: !56)
!192 = !DILocalVariable(name: "ret", scope: !189, file: !3, line: 91, type: !68)
!193 = distinct !DIAssignID()
!194 = !DILocation(line: 0, scope: !189)
!195 = !DILocation(line: 91, column: 9, scope: !189)
!196 = !DILocation(line: 91, column: 22, scope: !189)
!197 = distinct !DIAssignID()
!198 = !DILocation(line: 93, column: 14, scope: !199)
!199 = distinct !DILexicalBlock(scope: !189, file: !3, line: 93, column: 13)
!200 = !DILocation(line: 93, column: 13, scope: !199)
!201 = !DILocation(line: 95, column: 16, scope: !189)
!202 = !DILocation(line: 95, column: 9, scope: !189)
!203 = !DILocation(line: 96, column: 1, scope: !189)
!204 = distinct !DISubprogram(name: "prog9", scope: !3, file: !3, line: 98, type: !54, scopeLine: 98, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !205)
!205 = !{!206, !207}
!206 = !DILocalVariable(name: "ctx", arg: 1, scope: !204, file: !3, line: 98, type: !56)
!207 = !DILocalVariable(name: "ret", scope: !204, file: !3, line: 99, type: !68)
!208 = distinct !DIAssignID()
!209 = !DILocation(line: 0, scope: !204)
!210 = !DILocation(line: 99, column: 9, scope: !204)
!211 = !DILocation(line: 99, column: 22, scope: !204)
!212 = distinct !DIAssignID()
!213 = !DILocation(line: 101, column: 14, scope: !214)
!214 = distinct !DILexicalBlock(scope: !204, file: !3, line: 101, column: 13)
!215 = !DILocation(line: 101, column: 13, scope: !214)
!216 = !DILocation(line: 103, column: 16, scope: !204)
!217 = !DILocation(line: 103, column: 9, scope: !204)
!218 = !DILocation(line: 104, column: 1, scope: !204)
!219 = distinct !DISubprogram(name: "compat_test", scope: !3, file: !3, line: 108, type: !54, scopeLine: 108, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !220)
!220 = !{!221, !222}
!221 = !DILocalVariable(name: "ctx", arg: 1, scope: !219, file: !3, line: 108, type: !56)
!222 = !DILocalVariable(name: "ret", scope: !219, file: !3, line: 109, type: !68)
!223 = distinct !DIAssignID()
!224 = !DILocation(line: 0, scope: !219)
!225 = !DILocation(line: 109, column: 9, scope: !219)
!226 = !DILocation(line: 109, column: 22, scope: !219)
!227 = distinct !DIAssignID()
!228 = !DILocation(line: 111, column: 14, scope: !229)
!229 = distinct !DILexicalBlock(scope: !219, file: !3, line: 111, column: 13)
!230 = !DILocation(line: 111, column: 13, scope: !229)
!231 = !DILocation(line: 113, column: 16, scope: !219)
!232 = !DILocation(line: 113, column: 9, scope: !219)
!233 = !DILocation(line: 114, column: 1, scope: !219)
!234 = distinct !DISubprogram(name: "xdp_dispatcher", scope: !3, file: !3, line: 118, type: !54, scopeLine: 119, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !235)
!235 = !{!236, !237, !238, !239}
!236 = !DILocalVariable(name: "ctx", arg: 1, scope: !234, file: !3, line: 118, type: !56)
!237 = !DILocalVariable(name: "num_progs_enabled", scope: !234, file: !3, line: 120, type: !34)
!238 = !DILocalVariable(name: "ret", scope: !234, file: !3, line: 121, type: !25)
!239 = !DILabel(scope: !234, name: "out", file: !3, line: 189, column: 1)
!240 = !DILocation(line: 0, scope: !234)
!241 = !DILocation(line: 120, column: 39, scope: !234)
!242 = !{!243, !75, i64 2}
!243 = !{!"xdp_dispatcher_config", !75, i64 0, !75, i64 1, !75, i64 2, !75, i64 3, !75, i64 4, !75, i64 44, !75, i64 84}
!244 = !DILocation(line: 123, column: 31, scope: !245)
!245 = distinct !DILexicalBlock(scope: !234, file: !3, line: 123, column: 13)
!246 = !DILocation(line: 125, column: 15, scope: !234)
!247 = !DILocation(line: 126, column: 19, scope: !248)
!248 = distinct !DILexicalBlock(scope: !234, file: !3, line: 126, column: 13)
!249 = !DILocation(line: 126, column: 29, scope: !248)
!250 = !DILocation(line: 126, column: 27, scope: !248)
!251 = !DILocation(line: 126, column: 13, scope: !248)
!252 = !DILocation(line: 129, column: 31, scope: !253)
!253 = distinct !DILexicalBlock(scope: !234, file: !3, line: 129, column: 13)
!254 = !DILocation(line: 131, column: 15, scope: !234)
!255 = !DILocation(line: 132, column: 19, scope: !256)
!256 = distinct !DILexicalBlock(scope: !234, file: !3, line: 132, column: 13)
!257 = !DILocation(line: 132, column: 29, scope: !256)
!258 = !DILocation(line: 132, column: 27, scope: !256)
!259 = !DILocation(line: 132, column: 13, scope: !256)
!260 = !DILocation(line: 135, column: 31, scope: !261)
!261 = distinct !DILexicalBlock(scope: !234, file: !3, line: 135, column: 13)
!262 = !DILocation(line: 137, column: 15, scope: !234)
!263 = !DILocation(line: 138, column: 19, scope: !264)
!264 = distinct !DILexicalBlock(scope: !234, file: !3, line: 138, column: 13)
!265 = !DILocation(line: 138, column: 29, scope: !264)
!266 = !DILocation(line: 138, column: 27, scope: !264)
!267 = !DILocation(line: 138, column: 13, scope: !264)
!268 = !DILocation(line: 141, column: 31, scope: !269)
!269 = distinct !DILexicalBlock(scope: !234, file: !3, line: 141, column: 13)
!270 = !DILocation(line: 143, column: 15, scope: !234)
!271 = !DILocation(line: 144, column: 19, scope: !272)
!272 = distinct !DILexicalBlock(scope: !234, file: !3, line: 144, column: 13)
!273 = !DILocation(line: 144, column: 29, scope: !272)
!274 = !DILocation(line: 144, column: 27, scope: !272)
!275 = !DILocation(line: 144, column: 13, scope: !272)
!276 = !DILocation(line: 147, column: 31, scope: !277)
!277 = distinct !DILexicalBlock(scope: !234, file: !3, line: 147, column: 13)
!278 = !DILocation(line: 149, column: 15, scope: !234)
!279 = !DILocation(line: 150, column: 19, scope: !280)
!280 = distinct !DILexicalBlock(scope: !234, file: !3, line: 150, column: 13)
!281 = !DILocation(line: 150, column: 29, scope: !280)
!282 = !DILocation(line: 150, column: 27, scope: !280)
!283 = !DILocation(line: 150, column: 13, scope: !280)
!284 = !DILocation(line: 153, column: 31, scope: !285)
!285 = distinct !DILexicalBlock(scope: !234, file: !3, line: 153, column: 13)
!286 = !DILocation(line: 155, column: 15, scope: !234)
!287 = !DILocation(line: 156, column: 19, scope: !288)
!288 = distinct !DILexicalBlock(scope: !234, file: !3, line: 156, column: 13)
!289 = !DILocation(line: 156, column: 29, scope: !288)
!290 = !DILocation(line: 156, column: 27, scope: !288)
!291 = !DILocation(line: 156, column: 13, scope: !288)
!292 = !DILocation(line: 159, column: 31, scope: !293)
!293 = distinct !DILexicalBlock(scope: !234, file: !3, line: 159, column: 13)
!294 = !DILocation(line: 161, column: 15, scope: !234)
!295 = !DILocation(line: 162, column: 19, scope: !296)
!296 = distinct !DILexicalBlock(scope: !234, file: !3, line: 162, column: 13)
!297 = !DILocation(line: 162, column: 29, scope: !296)
!298 = !DILocation(line: 162, column: 27, scope: !296)
!299 = !DILocation(line: 162, column: 13, scope: !296)
!300 = !DILocation(line: 165, column: 31, scope: !301)
!301 = distinct !DILexicalBlock(scope: !234, file: !3, line: 165, column: 13)
!302 = !DILocation(line: 167, column: 15, scope: !234)
!303 = !DILocation(line: 168, column: 19, scope: !304)
!304 = distinct !DILexicalBlock(scope: !234, file: !3, line: 168, column: 13)
!305 = !DILocation(line: 168, column: 29, scope: !304)
!306 = !DILocation(line: 168, column: 27, scope: !304)
!307 = !DILocation(line: 168, column: 13, scope: !304)
!308 = !DILocation(line: 171, column: 31, scope: !309)
!309 = distinct !DILexicalBlock(scope: !234, file: !3, line: 171, column: 13)
!310 = !DILocation(line: 173, column: 15, scope: !234)
!311 = !DILocation(line: 174, column: 19, scope: !312)
!312 = distinct !DILexicalBlock(scope: !234, file: !3, line: 174, column: 13)
!313 = !DILocation(line: 174, column: 29, scope: !312)
!314 = !DILocation(line: 174, column: 27, scope: !312)
!315 = !DILocation(line: 174, column: 13, scope: !312)
!316 = !DILocation(line: 177, column: 31, scope: !317)
!317 = distinct !DILexicalBlock(scope: !234, file: !3, line: 177, column: 13)
!318 = !DILocation(line: 179, column: 15, scope: !234)
!319 = !DILocation(line: 180, column: 19, scope: !320)
!320 = distinct !DILexicalBlock(scope: !234, file: !3, line: 180, column: 13)
!321 = !DILocation(line: 180, column: 29, scope: !320)
!322 = !DILocation(line: 180, column: 27, scope: !320)
!323 = !DILocation(line: 180, column: 13, scope: !320)
!324 = !DILocation(line: 186, column: 31, scope: !325)
!325 = distinct !DILexicalBlock(scope: !234, file: !3, line: 186, column: 13)
!326 = !DILocation(line: 188, column: 15, scope: !234)
!327 = !DILocation(line: 188, column: 9, scope: !234)
!328 = !DILocation(line: 191, column: 1, scope: !234)
!329 = distinct !DISubprogram(name: "xdp_pass", scope: !3, file: !3, line: 194, type: !54, scopeLine: 195, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !330)
!330 = !{!331}
!331 = !DILocalVariable(name: "ctx", arg: 1, scope: !329, file: !3, line: 194, type: !56)
!332 = !DILocation(line: 0, scope: !329)
!333 = !DILocation(line: 196, column: 9, scope: !329)
