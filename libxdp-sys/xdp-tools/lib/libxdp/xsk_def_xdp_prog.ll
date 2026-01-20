; ModuleID = 'xsk_def_xdp_prog.c'
source_filename = "xsk_def_xdp_prog.c"
target datalayout = "e-m:e-p:64:64-i64:64-i128:128-n32:64-S128"
target triple = "bpf"

%struct.anon = type { ptr, ptr, ptr, ptr }
%struct.anon.0 = type { ptr, ptr }

@refcnt = dso_local global i32 1, align 4, !dbg !0
@xsks_map = dso_local global %struct.anon zeroinitializer, section ".maps", align 8, !dbg !33
@_license = dso_local global [4 x i8] c"GPL\00", section "license", align 1, !dbg !27
@_xsk_def_prog = dso_local global %struct.anon.0 zeroinitializer, section ".xdp_run_config", align 8, !dbg !52
@xsk_prog_version = dso_local global ptr null, section "xdp_metadata", align 8, !dbg !66
@llvm.compiler.used = appending global [5 x ptr] [ptr @_license, ptr @_xsk_def_prog, ptr @xsk_def_prog, ptr @xsk_prog_version, ptr @xsks_map], section "llvm.metadata"

; Function Attrs: nounwind
define dso_local i32 @xsk_def_prog(ptr noundef readonly captures(none) %0) #0 section "xdp" !dbg !75 {
    #dbg_value(ptr %0, !89, !DIExpression(), !90)
  %2 = load volatile i32, ptr @refcnt, align 4, !dbg !91, !tbaa !93
  %3 = icmp eq i32 %2, 0, !dbg !91
  br i1 %3, label %10, label %4, !dbg !97

4:                                                ; preds = %1
  %5 = getelementptr inbounds nuw i8, ptr %0, i64 16, !dbg !98
  %6 = load i32, ptr %5, align 4, !dbg !98, !tbaa !99
  %7 = zext i32 %6 to i64, !dbg !101
  %8 = tail call i64 inttoptr (i64 51 to ptr)(ptr noundef nonnull @xsks_map, i64 noundef %7, i64 noundef 2) #1, !dbg !102
  %9 = trunc i64 %8 to i32, !dbg !102
  br label %10, !dbg !103

10:                                               ; preds = %1, %4
  %11 = phi i32 [ %9, %4 ], [ 2, %1 ], !dbg !90
  ret i32 %11, !dbg !104
}

attributes #0 = { nounwind "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nounwind }

!llvm.dbg.cu = !{!2}
!llvm.module.flags = !{!69, !70, !71, !72, !73}
!llvm.ident = !{!74}

!0 = !DIGlobalVariableExpression(var: !1, expr: !DIExpression())
!1 = distinct !DIGlobalVariable(name: "refcnt", scope: !2, file: !3, line: 27, type: !68, isLocal: false, isDefinition: true)
!2 = distinct !DICompileUnit(language: DW_LANG_C11, file: !3, producer: "clang version 21.1.7 (Fedora 21.1.7-1.fc43)", isOptimized: true, runtimeVersion: 0, emissionKind: FullDebug, enums: !4, globals: !14, splitDebugInlining: false, nameTableKind: None)
!3 = !DIFile(filename: "xsk_def_xdp_prog.c", directory: "/var/home/leonardogiovannoni/Src/Net/original/wowow3223o/libxdp-sys/xdp-tools/lib/libxdp", checksumkind: CSK_MD5, checksum: "bce97ec18904cb0cd5d271e48243b250")
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
!14 = !{!0, !15, !27, !33, !52, !66}
!15 = !DIGlobalVariableExpression(var: !16, expr: !DIExpression())
!16 = distinct !DIGlobalVariable(name: "bpf_redirect_map", scope: !2, file: !17, line: 1325, type: !18, isLocal: true, isDefinition: true)
!17 = !DIFile(filename: "../libbpf/src/root/usr/include/bpf/bpf_helper_defs.h", directory: "/var/home/leonardogiovannoni/Src/Net/original/wowow3223o/libxdp-sys/xdp-tools/lib/libxdp", checksumkind: CSK_MD5, checksum: "65e4dc8e3121f91a5c2c9eb8563c5692")
!18 = !DIDerivedType(tag: DW_TAG_const_type, baseType: !19)
!19 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !20, size: 64)
!20 = !DISubroutineType(types: !21)
!21 = !{!22, !23, !24, !24}
!22 = !DIBasicType(name: "long", size: 64, encoding: DW_ATE_signed)
!23 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: null, size: 64)
!24 = !DIDerivedType(tag: DW_TAG_typedef, name: "__u64", file: !25, line: 31, baseType: !26)
!25 = !DIFile(filename: "/usr/include/asm-generic/int-ll64.h", directory: "", checksumkind: CSK_MD5, checksum: "b810f270733e106319b67ef512c6246e")
!26 = !DIBasicType(name: "unsigned long long", size: 64, encoding: DW_ATE_unsigned)
!27 = !DIGlobalVariableExpression(var: !28, expr: !DIExpression())
!28 = distinct !DIGlobalVariable(name: "_license", scope: !2, file: !3, line: 43, type: !29, isLocal: false, isDefinition: true)
!29 = !DICompositeType(tag: DW_TAG_array_type, baseType: !30, size: 32, elements: !31)
!30 = !DIBasicType(name: "char", size: 8, encoding: DW_ATE_signed_char)
!31 = !{!32}
!32 = !DISubrange(count: 4)
!33 = !DIGlobalVariableExpression(var: !34, expr: !DIExpression())
!34 = distinct !DIGlobalVariable(name: "xsks_map", scope: !2, file: !3, line: 16, type: !35, isLocal: false, isDefinition: true)
!35 = distinct !DICompositeType(tag: DW_TAG_structure_type, file: !3, line: 11, size: 256, elements: !36)
!36 = !{!37, !43, !46, !47}
!37 = !DIDerivedType(tag: DW_TAG_member, name: "type", scope: !35, file: !3, line: 12, baseType: !38, size: 64)
!38 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !39, size: 64)
!39 = !DICompositeType(tag: DW_TAG_array_type, baseType: !40, size: 544, elements: !41)
!40 = !DIBasicType(name: "int", size: 32, encoding: DW_ATE_signed)
!41 = !{!42}
!42 = !DISubrange(count: 17)
!43 = !DIDerivedType(tag: DW_TAG_member, name: "key_size", scope: !35, file: !3, line: 13, baseType: !44, size: 64, offset: 64)
!44 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !45, size: 64)
!45 = !DICompositeType(tag: DW_TAG_array_type, baseType: !40, size: 128, elements: !31)
!46 = !DIDerivedType(tag: DW_TAG_member, name: "value_size", scope: !35, file: !3, line: 14, baseType: !44, size: 64, offset: 128)
!47 = !DIDerivedType(tag: DW_TAG_member, name: "max_entries", scope: !35, file: !3, line: 15, baseType: !48, size: 64, offset: 192)
!48 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !49, size: 64)
!49 = !DICompositeType(tag: DW_TAG_array_type, baseType: !40, size: 2048, elements: !50)
!50 = !{!51}
!51 = !DISubrange(count: 64)
!52 = !DIGlobalVariableExpression(var: !53, expr: !DIExpression())
!53 = distinct !DIGlobalVariable(name: "_xsk_def_prog", scope: !2, file: !3, line: 21, type: !54, isLocal: false, isDefinition: true)
!54 = distinct !DICompositeType(tag: DW_TAG_structure_type, file: !3, line: 18, size: 128, elements: !55)
!55 = !{!56, !61}
!56 = !DIDerivedType(tag: DW_TAG_member, name: "priority", scope: !54, file: !3, line: 19, baseType: !57, size: 64)
!57 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !58, size: 64)
!58 = !DICompositeType(tag: DW_TAG_array_type, baseType: !40, size: 640, elements: !59)
!59 = !{!60}
!60 = !DISubrange(count: 20)
!61 = !DIDerivedType(tag: DW_TAG_member, name: "XDP_PASS", scope: !54, file: !3, line: 20, baseType: !62, size: 64, offset: 64)
!62 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !63, size: 64)
!63 = !DICompositeType(tag: DW_TAG_array_type, baseType: !40, size: 32, elements: !64)
!64 = !{!65}
!65 = !DISubrange(count: 1)
!66 = !DIGlobalVariableExpression(var: !67, expr: !DIExpression())
!67 = distinct !DIGlobalVariable(name: "xsk_prog_version", scope: !2, file: !3, line: 44, type: !62, isLocal: false, isDefinition: true)
!68 = !DIDerivedType(tag: DW_TAG_volatile_type, baseType: !40)
!69 = !{i32 7, !"Dwarf Version", i32 5}
!70 = !{i32 2, !"Debug Info Version", i32 3}
!71 = !{i32 1, !"wchar_size", i32 4}
!72 = !{i32 7, !"frame-pointer", i32 2}
!73 = !{i32 7, !"debug-info-assignment-tracking", i1 true}
!74 = !{!"clang version 21.1.7 (Fedora 21.1.7-1.fc43)"}
!75 = distinct !DISubprogram(name: "xsk_def_prog", scope: !3, file: !3, line: 31, type: !76, scopeLine: 32, flags: DIFlagPrototyped | DIFlagAllCallsDescribed, spFlags: DISPFlagDefinition | DISPFlagOptimized, unit: !2, retainedNodes: !88)
!76 = !DISubroutineType(types: !77)
!77 = !{!40, !78}
!78 = !DIDerivedType(tag: DW_TAG_pointer_type, baseType: !79, size: 64)
!79 = distinct !DICompositeType(tag: DW_TAG_structure_type, name: "xdp_md", file: !6, line: 5944, size: 192, elements: !80)
!80 = !{!81, !83, !84, !85, !86, !87}
!81 = !DIDerivedType(tag: DW_TAG_member, name: "data", scope: !79, file: !6, line: 5945, baseType: !82, size: 32)
!82 = !DIDerivedType(tag: DW_TAG_typedef, name: "__u32", file: !25, line: 27, baseType: !7)
!83 = !DIDerivedType(tag: DW_TAG_member, name: "data_end", scope: !79, file: !6, line: 5946, baseType: !82, size: 32, offset: 32)
!84 = !DIDerivedType(tag: DW_TAG_member, name: "data_meta", scope: !79, file: !6, line: 5947, baseType: !82, size: 32, offset: 64)
!85 = !DIDerivedType(tag: DW_TAG_member, name: "ingress_ifindex", scope: !79, file: !6, line: 5949, baseType: !82, size: 32, offset: 96)
!86 = !DIDerivedType(tag: DW_TAG_member, name: "rx_queue_index", scope: !79, file: !6, line: 5950, baseType: !82, size: 32, offset: 128)
!87 = !DIDerivedType(tag: DW_TAG_member, name: "egress_ifindex", scope: !79, file: !6, line: 5952, baseType: !82, size: 32, offset: 160)
!88 = !{!89}
!89 = !DILocalVariable(name: "ctx", arg: 1, scope: !75, file: !3, line: 31, type: !78)
!90 = !DILocation(line: 0, scope: !75)
!91 = !DILocation(line: 34, column: 7, scope: !92)
!92 = distinct !DILexicalBlock(scope: !75, file: !3, line: 34, column: 6)
!93 = !{!94, !94, i64 0}
!94 = !{!"int", !95, i64 0}
!95 = !{!"omnipotent char", !96, i64 0}
!96 = !{!"Simple C/C++ TBAA"}
!97 = !DILocation(line: 34, column: 6, scope: !92)
!98 = !DILocation(line: 40, column: 42, scope: !75)
!99 = !{!100, !94, i64 16}
!100 = !{!"xdp_md", !94, i64 0, !94, i64 4, !94, i64 8, !94, i64 12, !94, i64 16, !94, i64 20}
!101 = !DILocation(line: 40, column: 37, scope: !75)
!102 = !DILocation(line: 40, column: 9, scope: !75)
!103 = !DILocation(line: 40, column: 2, scope: !75)
!104 = !DILocation(line: 41, column: 1, scope: !75)
