
benchmarks/print_loop_c:     file format elf64-x86-64


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

0000000000001020 <fprintf@plt-0x10>:
    1020:	ff 35 ca 2f 00 00    	push   0x2fca(%rip)        # 3ff0 <_GLOBAL_OFFSET_TABLE_+0x8>
    1026:	ff 25 cc 2f 00 00    	jmp    *0x2fcc(%rip)        # 3ff8 <_GLOBAL_OFFSET_TABLE_+0x10>
    102c:	0f 1f 40 00          	nopl   0x0(%rax)

0000000000001030 <fprintf@plt>:
    1030:	ff 25 ca 2f 00 00    	jmp    *0x2fca(%rip)        # 4000 <fprintf@GLIBC_2.2.5>
    1036:	68 00 00 00 00       	push   $0x0
    103b:	e9 e0 ff ff ff       	jmp    1020 <_init+0x20>

Disassembly of section .plt.got:

0000000000001040 <__cxa_finalize@plt>:
    1040:	ff 25 92 2f 00 00    	jmp    *0x2f92(%rip)        # 3fd8 <__cxa_finalize@GLIBC_2.2.5>
    1046:	66 90                	xchg   %ax,%ax

Disassembly of section .text:

0000000000001050 <set_fast_math>:
    1050:	f3 0f 1e fa          	endbr64
    1054:	55                   	push   %rbp
    1055:	48 89 e5             	mov    %rsp,%rbp
    1058:	0f ae 5d fc          	stmxcsr -0x4(%rbp)
    105c:	81 4d fc 40 80 00 00 	orl    $0x8040,-0x4(%rbp)
    1063:	0f ae 55 fc          	ldmxcsr -0x4(%rbp)
    1067:	5d                   	pop    %rbp
    1068:	c3                   	ret
    1069:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000001070 <_start>:
    1070:	f3 0f 1e fa          	endbr64
    1074:	31 ed                	xor    %ebp,%ebp
    1076:	49 89 d1             	mov    %rdx,%r9
    1079:	5e                   	pop    %rsi
    107a:	48 89 e2             	mov    %rsp,%rdx
    107d:	48 83 e4 f0          	and    $0xfffffffffffffff0,%rsp
    1081:	50                   	push   %rax
    1082:	54                   	push   %rsp
    1083:	45 31 c0             	xor    %r8d,%r8d
    1086:	31 c9                	xor    %ecx,%ecx
    1088:	48 8d 3d d1 00 00 00 	lea    0xd1(%rip),%rdi        # 1160 <main>
    108f:	ff 15 23 2f 00 00    	call   *0x2f23(%rip)        # 3fb8 <__libc_start_main@GLIBC_2.34>
    1095:	f4                   	hlt
    1096:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
    109d:	00 00 00 

00000000000010a0 <deregister_tm_clones>:
    10a0:	48 8d 3d 71 2f 00 00 	lea    0x2f71(%rip),%rdi        # 4018 <__TMC_END__>
    10a7:	48 8d 05 6a 2f 00 00 	lea    0x2f6a(%rip),%rax        # 4018 <__TMC_END__>
    10ae:	48 39 f8             	cmp    %rdi,%rax
    10b1:	74 15                	je     10c8 <deregister_tm_clones+0x28>
    10b3:	48 8b 05 06 2f 00 00 	mov    0x2f06(%rip),%rax        # 3fc0 <_ITM_deregisterTMCloneTable@Base>
    10ba:	48 85 c0             	test   %rax,%rax
    10bd:	74 09                	je     10c8 <deregister_tm_clones+0x28>
    10bf:	ff e0                	jmp    *%rax
    10c1:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)
    10c8:	c3                   	ret
    10c9:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

00000000000010d0 <register_tm_clones>:
    10d0:	48 8d 3d 41 2f 00 00 	lea    0x2f41(%rip),%rdi        # 4018 <__TMC_END__>
    10d7:	48 8d 35 3a 2f 00 00 	lea    0x2f3a(%rip),%rsi        # 4018 <__TMC_END__>
    10de:	48 29 fe             	sub    %rdi,%rsi
    10e1:	48 89 f0             	mov    %rsi,%rax
    10e4:	48 c1 ee 3f          	shr    $0x3f,%rsi
    10e8:	48 c1 f8 03          	sar    $0x3,%rax
    10ec:	48 01 c6             	add    %rax,%rsi
    10ef:	48 d1 fe             	sar    $1,%rsi
    10f2:	74 14                	je     1108 <register_tm_clones+0x38>
    10f4:	48 8b 05 d5 2e 00 00 	mov    0x2ed5(%rip),%rax        # 3fd0 <_ITM_registerTMCloneTable@Base>
    10fb:	48 85 c0             	test   %rax,%rax
    10fe:	74 08                	je     1108 <register_tm_clones+0x38>
    1100:	ff e0                	jmp    *%rax
    1102:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)
    1108:	c3                   	ret
    1109:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000001110 <__do_global_dtors_aux>:
    1110:	f3 0f 1e fa          	endbr64
    1114:	80 3d fd 2e 00 00 00 	cmpb   $0x0,0x2efd(%rip)        # 4018 <__TMC_END__>
    111b:	75 2b                	jne    1148 <__do_global_dtors_aux+0x38>
    111d:	55                   	push   %rbp
    111e:	48 83 3d b2 2e 00 00 	cmpq   $0x0,0x2eb2(%rip)        # 3fd8 <__cxa_finalize@GLIBC_2.2.5>
    1125:	00 
    1126:	48 89 e5             	mov    %rsp,%rbp
    1129:	74 0c                	je     1137 <__do_global_dtors_aux+0x27>
    112b:	48 8b 3d de 2e 00 00 	mov    0x2ede(%rip),%rdi        # 4010 <__dso_handle>
    1132:	e8 09 ff ff ff       	call   1040 <__cxa_finalize@plt>
    1137:	e8 64 ff ff ff       	call   10a0 <deregister_tm_clones>
    113c:	c6 05 d5 2e 00 00 01 	movb   $0x1,0x2ed5(%rip)        # 4018 <__TMC_END__>
    1143:	5d                   	pop    %rbp
    1144:	c3                   	ret
    1145:	0f 1f 00             	nopl   (%rax)
    1148:	c3                   	ret
    1149:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000001150 <frame_dummy>:
    1150:	f3 0f 1e fa          	endbr64
    1154:	e9 77 ff ff ff       	jmp    10d0 <register_tm_clones>
    1159:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000001160 <main>:
    1160:	41 57                	push   %r15
    1162:	41 56                	push   %r14
    1164:	53                   	push   %rbx
    1165:	bb 01 00 00 00       	mov    $0x1,%ebx
    116a:	4c 8b 3d 6f 2e 00 00 	mov    0x2e6f(%rip),%r15        # 3fe0 <stderr@GLIBC_2.2.5>
    1171:	4c 8d 35 8c 0e 00 00 	lea    0xe8c(%rip),%r14        # 2004 <_IO_stdin_used+0x4>
    1178:	eb 12                	jmp    118c <main+0x2c>
    117a:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)
    1180:	48 ff c3             	inc    %rbx
    1183:	48 81 fb 81 f0 fa 02 	cmp    $0x2faf081,%rbx
    118a:	74 2c                	je     11b8 <main+0x58>
    118c:	89 d8                	mov    %ebx,%eax
    118e:	c1 e8 05             	shr    $0x5,%eax
    1191:	48 69 c0 c5 5a 7c 0a 	imul   $0xa7c5ac5,%rax,%rax
    1198:	48 c1 e8 27          	shr    $0x27,%rax
    119c:	69 c0 60 79 fe ff    	imul   $0xfffe7960,%eax,%eax
    11a2:	01 d8                	add    %ebx,%eax
    11a4:	75 da                	jne    1180 <main+0x20>
    11a6:	49 8b 3f             	mov    (%r15),%rdi
    11a9:	4c 89 f6             	mov    %r14,%rsi
    11ac:	48 89 da             	mov    %rbx,%rdx
    11af:	31 c0                	xor    %eax,%eax
    11b1:	e8 7a fe ff ff       	call   1030 <fprintf@plt>
    11b6:	eb c8                	jmp    1180 <main+0x20>
    11b8:	31 c0                	xor    %eax,%eax
    11ba:	5b                   	pop    %rbx
    11bb:	41 5e                	pop    %r14
    11bd:	41 5f                	pop    %r15
    11bf:	c3                   	ret

Disassembly of section .fini:

00000000000011c0 <_fini>:
    11c0:	f3 0f 1e fa          	endbr64
    11c4:	48 83 ec 08          	sub    $0x8,%rsp
    11c8:	48 83 c4 08          	add    $0x8,%rsp
    11cc:	c3                   	ret
