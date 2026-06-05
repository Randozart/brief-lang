
benchmarks/float_math_c:     file format elf64-x86-64


Disassembly of section .init:

0000000000001000 <_init>:
    1000:	f3 0f 1e fa          	endbr64
    1004:	48 83 ec 08          	sub    $0x8,%rsp
    1008:	48 8b 05 c1 2f 00 00 	mov    0x2fc1(%rip),%rax        # 3fd0 <__gmon_start__@Base>
    100f:	48 85 c0             	test   %rax,%rax
    1012:	74 02                	je     1016 <_init+0x16>
    1014:	ff d0                	call   *%rax
    1016:	48 83 c4 08          	add    $0x8,%rsp
    101a:	c3                   	ret

Disassembly of section .plt:

0000000000001020 <getenv@plt-0x10>:
    1020:	ff 35 ca 2f 00 00    	push   0x2fca(%rip)        # 3ff0 <_GLOBAL_OFFSET_TABLE_+0x8>
    1026:	ff 25 cc 2f 00 00    	jmp    *0x2fcc(%rip)        # 3ff8 <_GLOBAL_OFFSET_TABLE_+0x10>
    102c:	0f 1f 40 00          	nopl   0x0(%rax)

0000000000001030 <getenv@plt>:
    1030:	ff 25 ca 2f 00 00    	jmp    *0x2fca(%rip)        # 4000 <getenv@GLIBC_2.2.5>
    1036:	68 00 00 00 00       	push   $0x0
    103b:	e9 e0 ff ff ff       	jmp    1020 <_init+0x20>

0000000000001040 <strtol@plt>:
    1040:	ff 25 c2 2f 00 00    	jmp    *0x2fc2(%rip)        # 4008 <strtol@GLIBC_2.2.5>
    1046:	68 01 00 00 00       	push   $0x1
    104b:	e9 d0 ff ff ff       	jmp    1020 <_init+0x20>

Disassembly of section .plt.got:

0000000000001050 <__cxa_finalize@plt>:
    1050:	ff 25 8a 2f 00 00    	jmp    *0x2f8a(%rip)        # 3fe0 <__cxa_finalize@GLIBC_2.2.5>
    1056:	66 90                	xchg   %ax,%ax

Disassembly of section .text:

0000000000001060 <set_fast_math>:
    1060:	f3 0f 1e fa          	endbr64
    1064:	55                   	push   %rbp
    1065:	48 89 e5             	mov    %rsp,%rbp
    1068:	0f ae 5d fc          	stmxcsr -0x4(%rbp)
    106c:	81 4d fc 40 80 00 00 	orl    $0x8040,-0x4(%rbp)
    1073:	0f ae 55 fc          	ldmxcsr -0x4(%rbp)
    1077:	5d                   	pop    %rbp
    1078:	c3                   	ret
    1079:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000001080 <_start>:
    1080:	f3 0f 1e fa          	endbr64
    1084:	31 ed                	xor    %ebp,%ebp
    1086:	49 89 d1             	mov    %rdx,%r9
    1089:	5e                   	pop    %rsi
    108a:	48 89 e2             	mov    %rsp,%rdx
    108d:	48 83 e4 f0          	and    $0xfffffffffffffff0,%rsp
    1091:	50                   	push   %rax
    1092:	54                   	push   %rsp
    1093:	45 31 c0             	xor    %r8d,%r8d
    1096:	31 c9                	xor    %ecx,%ecx
    1098:	48 8d 3d d1 00 00 00 	lea    0xd1(%rip),%rdi        # 1170 <main>
    109f:	ff 15 1b 2f 00 00    	call   *0x2f1b(%rip)        # 3fc0 <__libc_start_main@GLIBC_2.34>
    10a5:	f4                   	hlt
    10a6:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
    10ad:	00 00 00 

00000000000010b0 <deregister_tm_clones>:
    10b0:	48 8d 3d 69 2f 00 00 	lea    0x2f69(%rip),%rdi        # 4020 <__TMC_END__>
    10b7:	48 8d 05 62 2f 00 00 	lea    0x2f62(%rip),%rax        # 4020 <__TMC_END__>
    10be:	48 39 f8             	cmp    %rdi,%rax
    10c1:	74 15                	je     10d8 <deregister_tm_clones+0x28>
    10c3:	48 8b 05 fe 2e 00 00 	mov    0x2efe(%rip),%rax        # 3fc8 <_ITM_deregisterTMCloneTable@Base>
    10ca:	48 85 c0             	test   %rax,%rax
    10cd:	74 09                	je     10d8 <deregister_tm_clones+0x28>
    10cf:	ff e0                	jmp    *%rax
    10d1:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)
    10d8:	c3                   	ret
    10d9:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

00000000000010e0 <register_tm_clones>:
    10e0:	48 8d 3d 39 2f 00 00 	lea    0x2f39(%rip),%rdi        # 4020 <__TMC_END__>
    10e7:	48 8d 35 32 2f 00 00 	lea    0x2f32(%rip),%rsi        # 4020 <__TMC_END__>
    10ee:	48 29 fe             	sub    %rdi,%rsi
    10f1:	48 89 f0             	mov    %rsi,%rax
    10f4:	48 c1 ee 3f          	shr    $0x3f,%rsi
    10f8:	48 c1 f8 03          	sar    $0x3,%rax
    10fc:	48 01 c6             	add    %rax,%rsi
    10ff:	48 d1 fe             	sar    $1,%rsi
    1102:	74 14                	je     1118 <register_tm_clones+0x38>
    1104:	48 8b 05 cd 2e 00 00 	mov    0x2ecd(%rip),%rax        # 3fd8 <_ITM_registerTMCloneTable@Base>
    110b:	48 85 c0             	test   %rax,%rax
    110e:	74 08                	je     1118 <register_tm_clones+0x38>
    1110:	ff e0                	jmp    *%rax
    1112:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)
    1118:	c3                   	ret
    1119:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000001120 <__do_global_dtors_aux>:
    1120:	f3 0f 1e fa          	endbr64
    1124:	80 3d f5 2e 00 00 00 	cmpb   $0x0,0x2ef5(%rip)        # 4020 <__TMC_END__>
    112b:	75 2b                	jne    1158 <__do_global_dtors_aux+0x38>
    112d:	55                   	push   %rbp
    112e:	48 83 3d aa 2e 00 00 	cmpq   $0x0,0x2eaa(%rip)        # 3fe0 <__cxa_finalize@GLIBC_2.2.5>
    1135:	00 
    1136:	48 89 e5             	mov    %rsp,%rbp
    1139:	74 0c                	je     1147 <__do_global_dtors_aux+0x27>
    113b:	48 8b 3d d6 2e 00 00 	mov    0x2ed6(%rip),%rdi        # 4018 <__dso_handle>
    1142:	e8 09 ff ff ff       	call   1050 <__cxa_finalize@plt>
    1147:	e8 64 ff ff ff       	call   10b0 <deregister_tm_clones>
    114c:	c6 05 cd 2e 00 00 01 	movb   $0x1,0x2ecd(%rip)        # 4020 <__TMC_END__>
    1153:	5d                   	pop    %rbp
    1154:	c3                   	ret
    1155:	0f 1f 00             	nopl   (%rax)
    1158:	c3                   	ret
    1159:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000001160 <frame_dummy>:
    1160:	f3 0f 1e fa          	endbr64
    1164:	e9 77 ff ff ff       	jmp    10e0 <register_tm_clones>
    1169:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000001170 <main>:
    1170:	53                   	push   %rbx
    1171:	48 8d 3d c8 0e 00 00 	lea    0xec8(%rip),%rdi        # 2040 <_IO_stdin_used+0x40>
    1178:	e8 b3 fe ff ff       	call   1030 <getenv@plt>
    117d:	48 85 c0             	test   %rax,%rax
    1180:	74 33                	je     11b5 <main+0x45>
    1182:	31 db                	xor    %ebx,%ebx
    1184:	48 89 c7             	mov    %rax,%rdi
    1187:	31 f6                	xor    %esi,%esi
    1189:	ba 0a 00 00 00       	mov    $0xa,%edx
    118e:	e8 ad fe ff ff       	call   1040 <strtol@plt>
    1193:	48 85 c0             	test   %rax,%rax
    1196:	0f 8e 6a 01 00 00    	jle    1306 <main+0x196>
    119c:	48 83 f8 20          	cmp    $0x20,%rax
    11a0:	73 1e                	jae    11c0 <main+0x50>
    11a2:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    11a6:	31 c9                	xor    %ecx,%ecx
    11a8:	c5 d0 57 ed          	vxorps %xmm5,%xmm5,%xmm5
    11ac:	c5 f0 57 c9          	vxorps %xmm1,%xmm1,%xmm1
    11b0:	e9 0f 01 00 00       	jmp    12c4 <main+0x154>
    11b5:	b8 80 f0 fa 02       	mov    $0x2faf080,%eax
    11ba:	48 83 f8 20          	cmp    $0x20,%rax
    11be:	72 e2                	jb     11a2 <main+0x32>
    11c0:	48 b9 e0 ff ff ff ff 	movabs $0x7fffffffffffffe0,%rcx
    11c7:	ff ff 7f 
    11ca:	48 21 c1             	and    %rax,%rcx
    11cd:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    11d1:	c4 62 7d 18 0d 2a 0e 	vbroadcastss 0xe2a(%rip),%ymm9        # 2004 <_IO_stdin_used+0x4>
    11d8:	00 00 
    11da:	48 89 ca             	mov    %rcx,%rdx
    11dd:	c4 41 28 57 d2       	vxorps %xmm10,%xmm10,%xmm10
    11e2:	c4 41 20 57 db       	vxorps %xmm11,%xmm11,%xmm11
    11e7:	c4 41 18 57 e4       	vxorps %xmm12,%xmm12,%xmm12
    11ec:	c5 d0 57 ed          	vxorps %xmm5,%xmm5,%xmm5
    11f0:	c5 c8 57 f6          	vxorps %xmm6,%xmm6,%xmm6
    11f4:	c5 c0 57 ff          	vxorps %xmm7,%xmm7,%xmm7
    11f8:	c4 41 38 57 c0       	vxorps %xmm8,%xmm8,%xmm8
    11fd:	c5 f0 57 c9          	vxorps %xmm1,%xmm1,%xmm1
    1201:	c5 e8 57 d2          	vxorps %xmm2,%xmm2,%xmm2
    1205:	c5 e0 57 db          	vxorps %xmm3,%xmm3,%xmm3
    1209:	c5 d8 57 e4          	vxorps %xmm4,%xmm4,%xmm4
    120d:	0f 1f 00             	nopl   (%rax)
    1210:	c5 b4 58 c0          	vaddps %ymm0,%ymm9,%ymm0
    1214:	c4 41 2c 58 d1       	vaddps %ymm9,%ymm10,%ymm10
    1219:	c4 41 24 58 d9       	vaddps %ymm9,%ymm11,%ymm11
    121e:	c4 41 1c 58 e1       	vaddps %ymm9,%ymm12,%ymm12
    1223:	c5 b4 58 ed          	vaddps %ymm5,%ymm9,%ymm5
    1227:	c5 b4 58 f6          	vaddps %ymm6,%ymm9,%ymm6
    122b:	c5 b4 58 ff          	vaddps %ymm7,%ymm9,%ymm7
    122f:	c4 41 3c 58 c1       	vaddps %ymm9,%ymm8,%ymm8
    1234:	c5 b4 58 c9          	vaddps %ymm1,%ymm9,%ymm1
    1238:	c5 b4 58 d2          	vaddps %ymm2,%ymm9,%ymm2
    123c:	c5 b4 58 db          	vaddps %ymm3,%ymm9,%ymm3
    1240:	c5 b4 58 e4          	vaddps %ymm4,%ymm9,%ymm4
    1244:	48 83 c2 e0          	add    $0xffffffffffffffe0,%rdx
    1248:	75 c6                	jne    1210 <main+0xa0>
    124a:	c5 ac 58 c0          	vaddps %ymm0,%ymm10,%ymm0
    124e:	c5 a4 58 c0          	vaddps %ymm0,%ymm11,%ymm0
    1252:	c5 9c 58 c0          	vaddps %ymm0,%ymm12,%ymm0
    1256:	c4 c3 7d 19 c1 01    	vextractf128 $0x1,%ymm0,%xmm9
    125c:	c5 b0 58 c0          	vaddps %xmm0,%xmm9,%xmm0
    1260:	c5 79 c6 c8 01       	vshufpd $0x1,%xmm0,%xmm0,%xmm9
    1265:	c5 b0 58 c0          	vaddps %xmm0,%xmm9,%xmm0
    1269:	c5 7a 16 c8          	vmovshdup %xmm0,%xmm9
    126d:	c5 b2 58 c0          	vaddss %xmm0,%xmm9,%xmm0
    1271:	c5 cc 58 ed          	vaddps %ymm5,%ymm6,%ymm5
    1275:	c5 c4 58 ed          	vaddps %ymm5,%ymm7,%ymm5
    1279:	c5 bc 58 ed          	vaddps %ymm5,%ymm8,%ymm5
    127d:	c4 e3 7d 19 ee 01    	vextractf128 $0x1,%ymm5,%xmm6
    1283:	c5 d0 58 ee          	vaddps %xmm6,%xmm5,%xmm5
    1287:	c5 d1 c6 f5 01       	vshufpd $0x1,%xmm5,%xmm5,%xmm6
    128c:	c5 d0 58 ee          	vaddps %xmm6,%xmm5,%xmm5
    1290:	c5 fa 16 f5          	vmovshdup %xmm5,%xmm6
    1294:	c5 d2 58 ee          	vaddss %xmm6,%xmm5,%xmm5
    1298:	c5 ec 58 c9          	vaddps %ymm1,%ymm2,%ymm1
    129c:	c5 e4 58 c9          	vaddps %ymm1,%ymm3,%ymm1
    12a0:	c5 dc 58 c9          	vaddps %ymm1,%ymm4,%ymm1
    12a4:	c4 e3 7d 19 ca 01    	vextractf128 $0x1,%ymm1,%xmm2
    12aa:	c5 f0 58 ca          	vaddps %xmm2,%xmm1,%xmm1
    12ae:	c5 f1 c6 d1 01       	vshufpd $0x1,%xmm1,%xmm1,%xmm2
    12b3:	c5 f0 58 ca          	vaddps %xmm2,%xmm1,%xmm1
    12b7:	c5 fa 16 d1          	vmovshdup %xmm1,%xmm2
    12bb:	c5 f2 58 ca          	vaddss %xmm2,%xmm1,%xmm1
    12bf:	48 39 c8             	cmp    %rcx,%rax
    12c2:	74 2d                	je     12f1 <main+0x181>
    12c4:	48 89 c2             	mov    %rax,%rdx
    12c7:	48 29 ca             	sub    %rcx,%rdx
    12ca:	c5 fa 10 15 32 0d 00 	vmovss 0xd32(%rip),%xmm2        # 2004 <_IO_stdin_used+0x4>
    12d1:	00 
    12d2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    12d9:	1f 84 00 00 00 00 00 
    12e0:	c5 fa 58 c2          	vaddss %xmm2,%xmm0,%xmm0
    12e4:	c5 d2 58 ea          	vaddss %xmm2,%xmm5,%xmm5
    12e8:	c5 f2 58 ca          	vaddss %xmm2,%xmm1,%xmm1
    12ec:	48 ff ca             	dec    %rdx
    12ef:	75 ef                	jne    12e0 <main+0x170>
    12f1:	c5 f2 58 c0          	vaddss %xmm0,%xmm1,%xmm0
    12f5:	c4 e1 92 2a c8       	vcvtsi2ss %rax,%xmm13,%xmm1
    12fa:	c5 d2 58 c9          	vaddss %xmm1,%xmm5,%xmm1
    12fe:	c5 f2 58 c0          	vaddss %xmm0,%xmm1,%xmm0
    1302:	c5 fa 2c d8          	vcvttss2si %xmm0,%ebx
    1306:	89 d8                	mov    %ebx,%eax
    1308:	5b                   	pop    %rbx
    1309:	c5 f8 77             	vzeroupper
    130c:	c3                   	ret

Disassembly of section .fini:

0000000000001310 <_fini>:
    1310:	f3 0f 1e fa          	endbr64
    1314:	48 83 ec 08          	sub    $0x8,%rsp
    1318:	48 83 c4 08          	add    $0x8,%rsp
    131c:	c3                   	ret
