; ModuleID = 'xsk_def_xdp_prog_5.3.c'
source_filename = "xsk_def_xdp_prog_5.3.c"
target datalayout = "e-m:e-p:64:64-i64:64-i128:128-n32:64-S128"
target triple = "bpf"

%struct.anon = type { ptr, ptr, ptr, ptr }
%struct.anon.0 = type { ptr, ptr }

@refcnt = dso_local global i32 1, align 4, !dbg !0
@xsks_map = dso_local global %struct.anon zeroinitializer, section ".maps", align 8, !dbg !41
@_license = dso_local global [4 x i8] c"GPL\00", section "license", align 1, !dbg !35
@_xsk_def_prog = dso_local global %struct.anon.0 zeroinitializer, section ".xdp_run_config", align 8, !dbg !60
@xsk_prog_version = dso_local global ptr null, section "xdp_metadata", align 8, !dbg !74
@llvm.compiler.used = appending global [5 x ptr] [ptr @_license, ptr @_xsk_def_prog, ptr @xsk_def_prog, ptr @xsk_prog_version, ptr @xsks_map], section "llvm.metadata"

; Function Attrs: nounwind
define dso_local i32 @xsk_def_prog(ptr noundef readonly captures(none) %0) #0 section "xdp" !dbg !83 {
  %2 = alloca i32, align 4, !DIAssignID !99
    #dbg_assign(i1 poison, !98, !DIExpression(), !99, ptr %2, !DIExpression(), !100)
    #dbg_value(ptr %0, !97, !DIExpression(), !100)
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %2) #2, !dbg !101
  %3 = getelementptr inbounds nuw i8, ptr %0, i64 16, !dbg !102
  %4 = load i32, ptr %3, align 4, !dbg !102, !tbaa !103
  store i32 %4, ptr %2, align 4, !dbg !108, !tbaa !109, !DIAssignID !110
    #dbg_assign(i32 %4, !98, !DIExpression(), !110, ptr %2, !DIExpression(), !100)
  %5 = load volatile i32, ptr @refcnt, align 4, !dbg !111, !tbaa !109
  %6 = icmp eq i32 %5, 0, !dbg !111
  br i1 %6, label %15, label %7, !dbg !113

7:                                                ; preds = %1
  %8 = call ptr inttoptr (i64 1 to ptr)(ptr noundef nonnull @xsks_map, ptr noundef nonnull %2) #2, !dbg !114
  %9 = icmp eq ptr %8, null, !dbg !114
  br i1 %9, label %15, label %10, !dbg !114

10:                                               ; preds = %7
  %11 = load i32, ptr %2, align 4, !dbg !116, !tbaa !109
  %12 = sext i32 %11 to i64, !dbg !116
  %13 = call i64 inttoptr (i64 51 to ptr)(ptr noundef nonnull @xsks_map, i64 noundef %12, i64 noundef 0) #2, !dbg !117
  %14 = trunc i64 %13 to i32, !dbg !117
  br label %15, !dbg !118

15:                                               ; preds = %7, %1, %10
  %16 = phi i32 [ %14, %10 ], [ 2, %1 ], [ 2, %7 ], !dbg !100
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %2) #2, !dbg !119
  ret i32 %16, !dbg !119
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(i64 immarg, ptr captures(none)) #1

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(i64 immarg, ptr captures(none)) #1

attributes #0 = { nounwind "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

!llvm.dbg.cu = !{!2}
!llvm.module.flags = !{!77, !78, !79, !80, !81}
!llvm.ident = !{!82}

!0 = !DIGlobalVariableExpression(var: !1, expr: !DIExpression())
!1 = distinct !DIGlobalVariable(name: "refcnt", scope: !2, file: !3, line: 27, type: !76, isLocal: false, isDefinition: true)
!2 = distinct !DICompileUnit(language: DW_LANG_C11, file: !3, producer: "clang version 21.1.7 (Fedora 21.1.7-1.fc43)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, enums: !4, globals: !14, splitDebugInlining: false, nameTableKind: None)
!3 = !DIFile(filename: "xsk_def_xdp_prog_5.3.c", directory: "/var/home/leonardogiovannoni/Src/Net/original/wowow3223o/libxdp-sys/xdp-tools/lib/libxdp", checksumkind: CSK_MD5, checksum: "bae3de4712c516b7b8ed19a2257b01b6")
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
!14 = !{!0, !15, !25, !35, !41, !60, !74}
!15 = !DIGlobalVariableExpression(var: !16, expr: !DIExpression())
!16 = distinct !DIGlobalVariable(name: "bpf_map_lookup_elem", scope: !2, file: !17, line: 56, type: !18, isLocal: true, isDefinition: true)
!17 = !DIFile(filename: "../libbpf/src/root/usr/include/bpf/bpf_helper_defs.h", directory: "/var/home/leonardogiovannoni/Src/Net/original/wowow3223o/libxdp-sys/xdp-tools/lib/libxdp", checksumkind: CSK_MD5, checksum: "65e4dc8e3121f91a5c2c9eb8563c5692")
!18 = !DIDerivedType(tag: DW_TAG_const_type, baseType: !19)
!19 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !20, size: 64)
!20 = !DISubroutineType(types: !21)
!21 = !{!22, !22, !23}
!22 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: null, size: 64)
!23 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !24, size: 64)
!24 = !DIDerivedType(tag: DW_TAG_const_type, baseType: null)
!25 = !DIGlobalVariableExpression(var: !26, expr: !DIExpression())
!26 = distinct !DIGlobalVariable(name: "bpf_redirect_map", scope: !2, file: !17, line: 1325, type: !27, isLocal: true, isDefinition: true)
!27 = !DIDerivedType(tag: DW_TAG_const_type, baseType: !28)
!28 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !29, size: 64)
!29 = !DISubroutineType(types: !30)
!30 = !{!31, !22, !32, !32}
!31 = !DIBasicType(name: "long", size: 64, encoding: DW_ATE_signed)
!32 = !DIDerivedType(tag: DW_TAG_typedef, name: "__u64", file: !33, line: 31, baseType: !34)
!33 = !DIFile(filename: "/usr/include/asm-generic/int-ll64.h", directory: "", checksumkind: CSK_MD5, checksum: "b810f270733e106319b67ef512c6246e")
!34 = !DIBasicType(name: "unsigned long long", size: 64, encoding: DW_ATE_unsigned)
!35 = !DIGlobalVariableExpression(var: !36, expr: !DIExpression())
!36 = distinct !DIGlobalVariable(name: "_license", scope: !2, file: !3, line: 48, type: !37, isLocal: false, isDefinition: true)
!37 = !DICompositeType(tag: DW_TAG_array_type, baseType: !38, size: 32, elements: !39)
!38 = !DIBasicType(name: "char", size: 8, encoding: DW_ATE_signed_char)
!39 = !{!40}
!40 = !DISubrange(count: 4)
!41 = !DIGlobalVariableExpression(var: !42, expr: !DIExpression())
!42 = distinct !DIGlobalVariable(name: "xsks_map", scope: !2, file: !3, line: 16, type: !43, isLocal: false, isDefinition: true)
!43 = distinct !DICompositeType(tag: DW_TAG_structure_type, file: !3, line: 11, size: 256, elements: !44)
!44 = !{!45, !51, !54, !55}
!45 = !DIDerivedType(tag: DW_TAG_member, name: "type", scope: !43, file: !3, line: 12, baseType: !46, size: 64)
!46 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !47, size: 64)
!47 = !DICompositeType(tag: DW_TAG_array_type, baseType: !48, size: 544, elements: !49)
!48 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!49 = !{!50}
!50 = !DISubrange(count: 17)
!51 = !DIDerivedType(tag: DW_TAG_member, name: "key_size", scope: !43, file: !3, line: 13, baseType: !52, size: 64, offset: 64)
!52 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !53, size: 64)
!53 = !DICompositeType(tag: DW_TAG_array_type, baseType: !48, size: 128, elements: !39)
!54 = !DIDerivedType(tag: DW_TAG_member, name: "value_size", scope: !43, file: !3, line: 14, baseType: !52, size: 64, offset: 128)
!55 = !DIDerivedType(tag: DW_TAG_member, name: "max_entries", scope: !43, file: !3, line: 15, baseType: !56, size: 64, offset: 192)
!56 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !57, size: 64)
!57 = !DICompositeType(tag: DW_TAG_array_type, baseType: !48, size: 2048, elements: !58)
!58 = !{!59}
!59 = !DISubrange(count: 64)
!60 = !DIGlobalVariableExpression(var: !61, expr: !DIExpression())
!61 = distinct !DIGlobalVariable(name: "_xsk_def_prog", scope: !2, file: !3, line: 21, type: !62, isLocal: false, isDefinition: true)
!62 = distinct !DICompositeType(tag: DW_TAG_structure_type, file: !3, line: 18, size: 128, elements: !63)
!63 = !{!64, !69}
!64 = !DIDerivedType(tag: DW_TAG_member, name: "priority", scope: !62, file: !3, line: 19, baseType: !65, size: 64)
!65 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !66, size: 64)
!66 = !DICompositeType(tag: DW_TAG_array_type, baseType: !48, size: 640, elements: !67)
!67 = !{!68}
!68 = !DISubrange(count: 20)
!69 = !DIDerivedType(tag: DW_TAG_member, name: "XDP_PASS", scope: !62, file: !3, line: 20, baseType: !70, size: 64, offset: 64)
!70 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !71, size: 64)
!71 = !DICompositeType(tag: DW_TAG_array_type, baseType: !48, size: 32, elements: !72)
!72 = !{!73}
!73 = !DISubrange(count: 1)
!74 = !DIGlobalVariableExpression(var: !75, expr: !DIExpression())
!75 = distinct !DIGlobalVariable(name: "xsk_prog_version", scope: !2, file: !3, line: 49, type: !70, isLocal: false, isDefinition: true)
!76 = !DIDerivedType(tag: DW_TAG_volatile_type, baseType: !48)
!77 = !{i32 7, !"Dwarf Version", i32 5}
!78 = !{i32 2, !"Debug Info Version", i32 3}
!79 = !{i32 1, !"wchar_size", i32 4}
!80 = !{i32 7, !"frame-pointer", i32 2}
!81 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!82 = !{!"clang version 21.1.7 (Fedora 21.1.7-1.fc43)"}
!83 = distinct !DISubprogram(name: "xsk_def_prog", scope: !3, file: !3, line: 31, type: !84, scopeLine: 32, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !96)
!84 = !DISubroutineType(types: !85)
!85 = !{!48, !86}
!86 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !87, size: 64)
!87 = distinct !DICompositeType(tag: DW_TAG_structure_type, name: "xdp_md", file: !6, line: 5944, size: 192, elements: !88)
!88 = !{!89, !91, !92, !93, !94, !95}
!89 = !DIDerivedType(tag: DW_TAG_member, name: "data", scope: !87, file: !6, line: 5945, baseType: !90, size: 32)
!90 = !DIDerivedType(tag: DW_TAG_typedef, name: "__u32", file: !33, line: 27, baseType: !7)
!91 = !DIDerivedType(tag: DW_TAG_member, name: "data_end", scope: !87, file: !6, line: 5946, baseType: !90, size: 32, offset: 32)
!92 = !DIDerivedType(tag: DW_TAG_member, name: "data_meta", scope: !87, file: !6, line: 5947, baseType: !90, size: 32, offset: 64)
!93 = !DIDerivedType(tag: DW_TAG_member, name: "ingress_ifindex", scope: !87, file: !6, line: 5949, baseType: !90, size: 32, offset: 96)
!94 = !DIDerivedType(tag: DW_TAG_member, name: "rx_queue_index", scope: !87, file: !6, line: 5950, baseType: !90, size: 32, offset: 128)
!95 = !DIDerivedType(tag: DW_TAG_member, name: "egress_ifindex", scope: !87, file: !6, line: 5952, baseType: !90, size: 32, offset: 160)
!96 = !{!97, !98}
!97 = !DILocalVariable(name: "ctx", arg: 1, scope: !83, file: !3, line: 31, type: !86)
!98 = !DILocalVariable(name: "index", scope: !83, file: !3, line: 33, type: !48)
!99 = distinct !DIAssignID()
!100 = !DILocation(line: 0, scope: !83)
!101 = !DILocation(line: 33, column: 2, scope: !83)
!102 = !DILocation(line: 33, column: 19, scope: !83)
!103 = !{!104, !105, i64 16}
!104 = !{!"xdp_md", !105, i64 0, !105, i64 4, !105, i64 8, !105, i64 12, !105, i64 16, !105, i64 20}
!105 = !{!"int", !106, i64 0}
!106 = !{!"omnipotent char", !107, i64 0}
!107 = !{!"Simple C/C++ TBAA"}
!108 = !DILocation(line: 33, column: 6, scope: !83)
!109 = !{!105, !105, i64 0}
!110 = distinct !DIAssignID()
!111 = !DILocation(line: 36, column: 7, scope: !112)
!112 = distinct !DILexicalBlock(scope: !83, file: !3, line: 36, column: 6)
!113 = !DILocation(line: 36, column: 6, scope: !112)
!114 = !DILocation(line: 42, column: 6, scope: !115)
!115 = distinct !DILexicalBlock(scope: !83, file: !3, line: 42, column: 6)
!116 = !DILocation(line: 43, column: 38, scope: !115)
!117 = !DILocation(line: 43, column: 10, scope: !115)
!118 = !DILocation(line: 43, column: 3, scope: !115)
!119 = !DILocation(line: 46, column: 1, scope: !83)
