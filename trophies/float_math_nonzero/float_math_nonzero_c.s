
benchmarks/float_math_nonzero_c:     file format elf64-x86-64


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
    1170:	50                   	push   %rax
    1171:	48 8d 3d a4 0e 00 00 	lea    0xea4(%rip),%rdi        # 201c <_IO_stdin_used+0x1c>
    1178:	e8 b3 fe ff ff       	call   1030 <getenv@plt>
    117d:	48 85 c0             	test   %rax,%rax
    1180:	74 1b                	je     119d <main+0x2d>
    1182:	48 89 c7             	mov    %rax,%rdi
    1185:	31 f6                	xor    %esi,%esi
    1187:	ba 0a 00 00 00       	mov    $0xa,%edx
    118c:	e8 af fe ff ff       	call   1040 <strtol@plt>
    1191:	48 85 c0             	test   %rax,%rax
    1194:	7f 0c                	jg     11a2 <main+0x32>
    1196:	b8 01 00 00 00       	mov    $0x1,%eax
    119b:	59                   	pop    %rcx
    119c:	c3                   	ret
    119d:	b8 80 f0 fa 02       	mov    $0x2faf080,%eax
    11a2:	c5 e8 57 d2          	vxorps %xmm2,%xmm2,%xmm2
    11a6:	c5 fa 10 0d 56 0e 00 	vmovss 0xe56(%rip),%xmm1        # 2004 <_IO_stdin_used+0x4>
    11ad:	00 
    11ae:	c5 fa 10 05 52 0e 00 	vmovss 0xe52(%rip),%xmm0        # 2008 <_IO_stdin_used+0x8>
    11b5:	00 
    11b6:	c5 fa 10 35 4e 0e 00 	vmovss 0xe4e(%rip),%xmm6        # 200c <_IO_stdin_used+0xc>
    11bd:	00 
    11be:	c5 fa 10 1d 4a 0e 00 	vmovss 0xe4a(%rip),%xmm3        # 2010 <_IO_stdin_used+0x10>
    11c5:	00 
    11c6:	c5 fa 10 25 46 0e 00 	vmovss 0xe46(%rip),%xmm4        # 2014 <_IO_stdin_used+0x14>
    11cd:	00 
    11ce:	c5 fa 10 2d 42 0e 00 	vmovss 0xe42(%rip),%xmm5        # 2018 <_IO_stdin_used+0x18>
    11d5:	00 
    11d6:	48 89 c1             	mov    %rax,%rcx
    11d9:	c5 c0 57 ff          	vxorps %xmm7,%xmm7,%xmm7
    11dd:	c4 41 38 57 c0       	vxorps %xmm8,%xmm8,%xmm8
    11e2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    11e9:	1f 84 00 00 00 00 00 
    11f0:	c5 7a 59 cb          	vmulss %xmm3,%xmm0,%xmm9
    11f4:	c5 72 59 d4          	vmulss %xmm4,%xmm1,%xmm10
    11f8:	c4 41 32 58 d2       	vaddss %xmm10,%xmm9,%xmm10
    11fd:	c5 4a 58 d9          	vaddss %xmm1,%xmm6,%xmm11
    1201:	c5 4a 59 e4          	vmulss %xmm4,%xmm6,%xmm12
    1205:	c5 aa 58 f6          	vaddss %xmm6,%xmm10,%xmm6
    1209:	c5 22 59 d3          	vmulss %xmm3,%xmm11,%xmm10
    120d:	c5 aa 58 c0          	vaddss %xmm0,%xmm10,%xmm0
    1211:	c5 b2 58 c9          	vaddss %xmm1,%xmm9,%xmm1
    1215:	c5 9a 58 c9          	vaddss %xmm1,%xmm12,%xmm1
    1219:	c5 ea 58 d5          	vaddss %xmm5,%xmm2,%xmm2
    121d:	c5 c2 58 fd          	vaddss %xmm5,%xmm7,%xmm7
    1221:	c5 3a 58 c5          	vaddss %xmm5,%xmm8,%xmm8
    1225:	48 ff c9             	dec    %rcx
    1228:	75 c6                	jne    11f0 <main+0x80>
    122a:	c4 e1 92 2a d8       	vcvtsi2ss %rax,%xmm13,%xmm3
    122f:	c5 ba 58 e7          	vaddss %xmm7,%xmm8,%xmm4
    1233:	c5 da 58 d2          	vaddss %xmm2,%xmm4,%xmm2
    1237:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
    123b:	c5 f2 58 c0          	vaddss %xmm0,%xmm1,%xmm0
    123f:	c5 ea 58 ce          	vaddss %xmm6,%xmm2,%xmm1
    1243:	c5 f2 58 c0          	vaddss %xmm0,%xmm1,%xmm0
    1247:	c5 fa 2c c0          	vcvttss2si %xmm0,%eax
    124b:	59                   	pop    %rcx
    124c:	c3                   	ret

Disassembly of section .fini:

0000000000001250 <_fini>:
    1250:	f3 0f 1e fa          	endbr64
    1254:	48 83 ec 08          	sub    $0x8,%rsp
    1258:	48 83 c4 08          	add    $0x8,%rsp
    125c:	c3                   	ret
