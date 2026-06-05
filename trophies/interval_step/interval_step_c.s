
benchmarks/interval_step_c:     file format elf64-x86-64


Disassembly of section .init:

0000000000001000 <_init>:
    1000:	f3 0f 1e fa          	endbr64
    1004:	48 83 ec 08          	sub    $0x8,%rsp
    1008:	48 8b 05 b9 2f 00 00 	mov    0x2fb9(%rip),%rax        # 3fc8 <__gmon_start__@Base>
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

0000000000001040 <fprintf@plt>:
    1040:	ff 25 c2 2f 00 00    	jmp    *0x2fc2(%rip)        # 4008 <fprintf@GLIBC_2.2.5>
    1046:	68 01 00 00 00       	push   $0x1
    104b:	e9 d0 ff ff ff       	jmp    1020 <_init+0x20>

0000000000001050 <strtol@plt>:
    1050:	ff 25 ba 2f 00 00    	jmp    *0x2fba(%rip)        # 4010 <strtol@GLIBC_2.2.5>
    1056:	68 02 00 00 00       	push   $0x2
    105b:	e9 c0 ff ff ff       	jmp    1020 <_init+0x20>

Disassembly of section .plt.got:

0000000000001060 <__cxa_finalize@plt>:
    1060:	ff 25 72 2f 00 00    	jmp    *0x2f72(%rip)        # 3fd8 <__cxa_finalize@GLIBC_2.2.5>
    1066:	66 90                	xchg   %ax,%ax

Disassembly of section .text:

0000000000001070 <set_fast_math>:
    1070:	f3 0f 1e fa          	endbr64
    1074:	55                   	push   %rbp
    1075:	48 89 e5             	mov    %rsp,%rbp
    1078:	0f ae 5d fc          	stmxcsr -0x4(%rbp)
    107c:	81 4d fc 40 80 00 00 	orl    $0x8040,-0x4(%rbp)
    1083:	0f ae 55 fc          	ldmxcsr -0x4(%rbp)
    1087:	5d                   	pop    %rbp
    1088:	c3                   	ret
    1089:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000001090 <_start>:
    1090:	f3 0f 1e fa          	endbr64
    1094:	31 ed                	xor    %ebp,%ebp
    1096:	49 89 d1             	mov    %rdx,%r9
    1099:	5e                   	pop    %rsi
    109a:	48 89 e2             	mov    %rsp,%rdx
    109d:	48 83 e4 f0          	and    $0xfffffffffffffff0,%rsp
    10a1:	50                   	push   %rax
    10a2:	54                   	push   %rsp
    10a3:	45 31 c0             	xor    %r8d,%r8d
    10a6:	31 c9                	xor    %ecx,%ecx
    10a8:	48 8d 3d d1 00 00 00 	lea    0xd1(%rip),%rdi        # 1180 <main>
    10af:	ff 15 03 2f 00 00    	call   *0x2f03(%rip)        # 3fb8 <__libc_start_main@GLIBC_2.34>
    10b5:	f4                   	hlt
    10b6:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
    10bd:	00 00 00 

00000000000010c0 <deregister_tm_clones>:
    10c0:	48 8d 3d 61 2f 00 00 	lea    0x2f61(%rip),%rdi        # 4028 <__TMC_END__>
    10c7:	48 8d 05 5a 2f 00 00 	lea    0x2f5a(%rip),%rax        # 4028 <__TMC_END__>
    10ce:	48 39 f8             	cmp    %rdi,%rax
    10d1:	74 15                	je     10e8 <deregister_tm_clones+0x28>
    10d3:	48 8b 05 e6 2e 00 00 	mov    0x2ee6(%rip),%rax        # 3fc0 <_ITM_deregisterTMCloneTable@Base>
    10da:	48 85 c0             	test   %rax,%rax
    10dd:	74 09                	je     10e8 <deregister_tm_clones+0x28>
    10df:	ff e0                	jmp    *%rax
    10e1:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)
    10e8:	c3                   	ret
    10e9:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

00000000000010f0 <register_tm_clones>:
    10f0:	48 8d 3d 31 2f 00 00 	lea    0x2f31(%rip),%rdi        # 4028 <__TMC_END__>
    10f7:	48 8d 35 2a 2f 00 00 	lea    0x2f2a(%rip),%rsi        # 4028 <__TMC_END__>
    10fe:	48 29 fe             	sub    %rdi,%rsi
    1101:	48 89 f0             	mov    %rsi,%rax
    1104:	48 c1 ee 3f          	shr    $0x3f,%rsi
    1108:	48 c1 f8 03          	sar    $0x3,%rax
    110c:	48 01 c6             	add    %rax,%rsi
    110f:	48 d1 fe             	sar    $1,%rsi
    1112:	74 14                	je     1128 <register_tm_clones+0x38>
    1114:	48 8b 05 b5 2e 00 00 	mov    0x2eb5(%rip),%rax        # 3fd0 <_ITM_registerTMCloneTable@Base>
    111b:	48 85 c0             	test   %rax,%rax
    111e:	74 08                	je     1128 <register_tm_clones+0x38>
    1120:	ff e0                	jmp    *%rax
    1122:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)
    1128:	c3                   	ret
    1129:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000001130 <__do_global_dtors_aux>:
    1130:	f3 0f 1e fa          	endbr64
    1134:	80 3d ed 2e 00 00 00 	cmpb   $0x0,0x2eed(%rip)        # 4028 <__TMC_END__>
    113b:	75 2b                	jne    1168 <__do_global_dtors_aux+0x38>
    113d:	55                   	push   %rbp
    113e:	48 83 3d 92 2e 00 00 	cmpq   $0x0,0x2e92(%rip)        # 3fd8 <__cxa_finalize@GLIBC_2.2.5>
    1145:	00 
    1146:	48 89 e5             	mov    %rsp,%rbp
    1149:	74 0c                	je     1157 <__do_global_dtors_aux+0x27>
    114b:	48 8b 3d ce 2e 00 00 	mov    0x2ece(%rip),%rdi        # 4020 <__dso_handle>
    1152:	e8 09 ff ff ff       	call   1060 <__cxa_finalize@plt>
    1157:	e8 64 ff ff ff       	call   10c0 <deregister_tm_clones>
    115c:	c6 05 c5 2e 00 00 01 	movb   $0x1,0x2ec5(%rip)        # 4028 <__TMC_END__>
    1163:	5d                   	pop    %rbp
    1164:	c3                   	ret
    1165:	0f 1f 00             	nopl   (%rax)
    1168:	c3                   	ret
    1169:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000001170 <frame_dummy>:
    1170:	f3 0f 1e fa          	endbr64
    1174:	e9 77 ff ff ff       	jmp    10f0 <register_tm_clones>
    1179:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000001180 <main>:
    1180:	55                   	push   %rbp
    1181:	41 57                	push   %r15
    1183:	41 56                	push   %r14
    1185:	41 55                	push   %r13
    1187:	41 54                	push   %r12
    1189:	53                   	push   %rbx
    118a:	50                   	push   %rax
    118b:	48 8d 3d 72 0e 00 00 	lea    0xe72(%rip),%rdi        # 2004 <_IO_stdin_used+0x4>
    1192:	e8 99 fe ff ff       	call   1030 <getenv@plt>
    1197:	48 85 c0             	test   %rax,%rax
    119a:	74 1c                	je     11b8 <main+0x38>
    119c:	45 31 f6             	xor    %r14d,%r14d
    119f:	48 89 c7             	mov    %rax,%rdi
    11a2:	31 f6                	xor    %esi,%esi
    11a4:	ba 0a 00 00 00       	mov    $0xa,%edx
    11a9:	e8 a2 fe ff ff       	call   1050 <strtol@plt>
    11ae:	48 89 c3             	mov    %rax,%rbx
    11b1:	48 85 c0             	test   %rax,%rax
    11b4:	7f 07                	jg     11bd <main+0x3d>
    11b6:	eb 6b                	jmp    1223 <main+0xa3>
    11b8:	bb 80 f0 fa 02       	mov    $0x2faf080,%ebx
    11bd:	41 bf 01 00 00 00    	mov    $0x1,%r15d
    11c3:	49 bd bd 42 7a e5 d5 	movabs $0xd6bf94d5e57a42bd,%r13
    11ca:	94 bf d6 
    11cd:	4c 8d 25 36 0e 00 00 	lea    0xe36(%rip),%r12        # 200a <_IO_stdin_used+0xa>
    11d4:	45 31 f6             	xor    %r14d,%r14d
    11d7:	31 ed                	xor    %ebp,%ebp
    11d9:	eb 10                	jmp    11eb <main+0x6b>
    11db:	0f 1f 44 00 00       	nopl   0x0(%rax,%rax,1)
    11e0:	48 ff c5             	inc    %rbp
    11e3:	49 ff c7             	inc    %r15
    11e6:	48 39 eb             	cmp    %rbp,%rbx
    11e9:	74 35                	je     1220 <main+0xa0>
    11eb:	4c 89 f8             	mov    %r15,%rax
    11ee:	49 f7 e5             	mul    %r13
    11f1:	48 c1 ea 16          	shr    $0x16,%rdx
    11f5:	48 69 c2 40 4b 4c 00 	imul   $0x4c4b40,%rdx,%rax
    11fc:	48 ff c8             	dec    %rax
    11ff:	49 01 ee             	add    %rbp,%r14
    1202:	48 39 e8             	cmp    %rbp,%rax
    1205:	75 d9                	jne    11e0 <main+0x60>
    1207:	48 8b 05 d2 2d 00 00 	mov    0x2dd2(%rip),%rax        # 3fe0 <stderr@GLIBC_2.2.5>
    120e:	48 8b 38             	mov    (%rax),%rdi
    1211:	4c 89 e6             	mov    %r12,%rsi
    1214:	4c 89 f2             	mov    %r14,%rdx
    1217:	31 c0                	xor    %eax,%eax
    1219:	e8 22 fe ff ff       	call   1040 <fprintf@plt>
    121e:	eb c0                	jmp    11e0 <main+0x60>
    1220:	41 01 de             	add    %ebx,%r14d
    1223:	44 89 f0             	mov    %r14d,%eax
    1226:	48 83 c4 08          	add    $0x8,%rsp
    122a:	5b                   	pop    %rbx
    122b:	41 5c                	pop    %r12
    122d:	41 5d                	pop    %r13
    122f:	41 5e                	pop    %r14
    1231:	41 5f                	pop    %r15
    1233:	5d                   	pop    %rbp
    1234:	c3                   	ret

Disassembly of section .fini:

0000000000001238 <_fini>:
    1238:	f3 0f 1e fa          	endbr64
    123c:	48 83 ec 08          	sub    $0x8,%rsp
    1240:	48 83 c4 08          	add    $0x8,%rsp
    1244:	c3                   	ret
