
benchmarks/float_math_nonzero:     file format elf64-x86-64


Disassembly of section .init:

0000000000401000 <_init>:
  401000:	f3 0f 1e fa          	endbr64
  401004:	48 83 ec 08          	sub    $0x8,%rsp
  401008:	48 8b 05 c9 2f 00 00 	mov    0x2fc9(%rip),%rax        # 403fd8 <__gmon_start__@Base>
  40100f:	48 85 c0             	test   %rax,%rax
  401012:	74 02                	je     401016 <_init+0x16>
  401014:	ff d0                	call   *%rax
  401016:	48 83 c4 08          	add    $0x8,%rsp
  40101a:	c3                   	ret

Disassembly of section .plt:

0000000000401020 <getenv@plt-0x10>:
  401020:	ff 35 ca 2f 00 00    	push   0x2fca(%rip)        # 403ff0 <_GLOBAL_OFFSET_TABLE_+0x8>
  401026:	ff 25 cc 2f 00 00    	jmp    *0x2fcc(%rip)        # 403ff8 <_GLOBAL_OFFSET_TABLE_+0x10>
  40102c:	0f 1f 40 00          	nopl   0x0(%rax)

0000000000401030 <getenv@plt>:
  401030:	ff 25 ca 2f 00 00    	jmp    *0x2fca(%rip)        # 404000 <getenv@GLIBC_2.2.5>
  401036:	68 00 00 00 00       	push   $0x0
  40103b:	e9 e0 ff ff ff       	jmp    401020 <_init+0x20>

0000000000401040 <timer_settime@plt>:
  401040:	ff 25 c2 2f 00 00    	jmp    *0x2fc2(%rip)        # 404008 <timer_settime@GLIBC_2.34>
  401046:	68 01 00 00 00       	push   $0x1
  40104b:	e9 d0 ff ff ff       	jmp    401020 <_init+0x20>

0000000000401050 <sigaction@plt>:
  401050:	ff 25 ba 2f 00 00    	jmp    *0x2fba(%rip)        # 404010 <sigaction@GLIBC_2.2.5>
  401056:	68 02 00 00 00       	push   $0x2
  40105b:	e9 c0 ff ff ff       	jmp    401020 <_init+0x20>

0000000000401060 <fread@plt>:
  401060:	ff 25 b2 2f 00 00    	jmp    *0x2fb2(%rip)        # 404018 <fread@GLIBC_2.2.5>
  401066:	68 03 00 00 00       	push   $0x3
  40106b:	e9 b0 ff ff ff       	jmp    401020 <_init+0x20>

0000000000401070 <timer_create@plt>:
  401070:	ff 25 aa 2f 00 00    	jmp    *0x2faa(%rip)        # 404020 <timer_create@GLIBC_2.34>
  401076:	68 04 00 00 00       	push   $0x4
  40107b:	e9 a0 ff ff ff       	jmp    401020 <_init+0x20>

0000000000401080 <fputs@plt>:
  401080:	ff 25 a2 2f 00 00    	jmp    *0x2fa2(%rip)        # 404028 <fputs@GLIBC_2.2.5>
  401086:	68 05 00 00 00       	push   $0x5
  40108b:	e9 90 ff ff ff       	jmp    401020 <_init+0x20>

0000000000401090 <epoll_ctl@plt>:
  401090:	ff 25 9a 2f 00 00    	jmp    *0x2f9a(%rip)        # 404030 <epoll_ctl@GLIBC_2.3.2>
  401096:	68 06 00 00 00       	push   $0x6
  40109b:	e9 80 ff ff ff       	jmp    401020 <_init+0x20>

00000000004010a0 <putc@plt>:
  4010a0:	ff 25 92 2f 00 00    	jmp    *0x2f92(%rip)        # 404038 <putc@GLIBC_2.2.5>
  4010a6:	68 07 00 00 00       	push   $0x7
  4010ab:	e9 70 ff ff ff       	jmp    401020 <_init+0x20>

00000000004010b0 <signal@plt>:
  4010b0:	ff 25 8a 2f 00 00    	jmp    *0x2f8a(%rip)        # 404040 <signal@GLIBC_2.2.5>
  4010b6:	68 08 00 00 00       	push   $0x8
  4010bb:	e9 60 ff ff ff       	jmp    401020 <_init+0x20>

00000000004010c0 <fprintf@plt>:
  4010c0:	ff 25 82 2f 00 00    	jmp    *0x2f82(%rip)        # 404048 <fprintf@GLIBC_2.2.5>
  4010c6:	68 09 00 00 00       	push   $0x9
  4010cb:	e9 50 ff ff ff       	jmp    401020 <_init+0x20>

00000000004010d0 <strtol@plt>:
  4010d0:	ff 25 7a 2f 00 00    	jmp    *0x2f7a(%rip)        # 404050 <strtol@GLIBC_2.2.5>
  4010d6:	68 0a 00 00 00       	push   $0xa
  4010db:	e9 40 ff ff ff       	jmp    401020 <_init+0x20>

00000000004010e0 <select@plt>:
  4010e0:	ff 25 72 2f 00 00    	jmp    *0x2f72(%rip)        # 404058 <select@GLIBC_2.2.5>
  4010e6:	68 0b 00 00 00       	push   $0xb
  4010eb:	e9 30 ff ff ff       	jmp    401020 <_init+0x20>

00000000004010f0 <sqrtf@plt>:
  4010f0:	ff 25 6a 2f 00 00    	jmp    *0x2f6a(%rip)        # 404060 <sqrtf@GLIBC_2.2.5>
  4010f6:	68 0c 00 00 00       	push   $0xc
  4010fb:	e9 20 ff ff ff       	jmp    401020 <_init+0x20>

0000000000401100 <epoll_wait@plt>:
  401100:	ff 25 62 2f 00 00    	jmp    *0x2f62(%rip)        # 404068 <epoll_wait@GLIBC_2.3.2>
  401106:	68 0d 00 00 00       	push   $0xd
  40110b:	e9 10 ff ff ff       	jmp    401020 <_init+0x20>

0000000000401110 <setvbuf@plt>:
  401110:	ff 25 5a 2f 00 00    	jmp    *0x2f5a(%rip)        # 404070 <setvbuf@GLIBC_2.2.5>
  401116:	68 0e 00 00 00       	push   $0xe
  40111b:	e9 00 ff ff ff       	jmp    401020 <_init+0x20>

0000000000401120 <__libc_current_sigrtmin@plt>:
  401120:	ff 25 52 2f 00 00    	jmp    *0x2f52(%rip)        # 404078 <__libc_current_sigrtmin@GLIBC_2.2.5>
  401126:	68 0f 00 00 00       	push   $0xf
  40112b:	e9 f0 fe ff ff       	jmp    401020 <_init+0x20>

0000000000401130 <exit@plt>:
  401130:	ff 25 4a 2f 00 00    	jmp    *0x2f4a(%rip)        # 404080 <exit@GLIBC_2.2.5>
  401136:	68 10 00 00 00       	push   $0x10
  40113b:	e9 e0 fe ff ff       	jmp    401020 <_init+0x20>

0000000000401140 <fwrite@plt>:
  401140:	ff 25 42 2f 00 00    	jmp    *0x2f42(%rip)        # 404088 <fwrite@GLIBC_2.2.5>
  401146:	68 11 00 00 00       	push   $0x11
  40114b:	e9 d0 fe ff ff       	jmp    401020 <_init+0x20>

0000000000401150 <epoll_create1@plt>:
  401150:	ff 25 3a 2f 00 00    	jmp    *0x2f3a(%rip)        # 404090 <epoll_create1@GLIBC_2.9>
  401156:	68 12 00 00 00       	push   $0x12
  40115b:	e9 c0 fe ff ff       	jmp    401020 <_init+0x20>

Disassembly of section .text:

0000000000401160 <_start>:
  401160:	f3 0f 1e fa          	endbr64
  401164:	31 ed                	xor    %ebp,%ebp
  401166:	49 89 d1             	mov    %rdx,%r9
  401169:	5e                   	pop    %rsi
  40116a:	48 89 e2             	mov    %rsp,%rdx
  40116d:	48 83 e4 f0          	and    $0xfffffffffffffff0,%rsp
  401171:	50                   	push   %rax
  401172:	54                   	push   %rsp
  401173:	45 31 c0             	xor    %r8d,%r8d
  401176:	31 c9                	xor    %ecx,%ecx
  401178:	48 c7 c7 b0 13 40 00 	mov    $0x4013b0,%rdi
  40117f:	ff 15 3b 2e 00 00    	call   *0x2e3b(%rip)        # 403fc0 <__libc_start_main@GLIBC_2.34>
  401185:	f4                   	hlt
  401186:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  40118d:	00 00 00 

0000000000401190 <_dl_relocate_static_pie>:
  401190:	f3 0f 1e fa          	endbr64
  401194:	c3                   	ret
  401195:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  40119c:	00 00 00 
  40119f:	90                   	nop

00000000004011a0 <deregister_tm_clones>:
  4011a0:	b8 b0 40 40 00       	mov    $0x4040b0,%eax
  4011a5:	48 3d b0 40 40 00    	cmp    $0x4040b0,%rax
  4011ab:	74 13                	je     4011c0 <deregister_tm_clones+0x20>
  4011ad:	b8 00 00 00 00       	mov    $0x0,%eax
  4011b2:	48 85 c0             	test   %rax,%rax
  4011b5:	74 09                	je     4011c0 <deregister_tm_clones+0x20>
  4011b7:	bf b0 40 40 00       	mov    $0x4040b0,%edi
  4011bc:	ff e0                	jmp    *%rax
  4011be:	66 90                	xchg   %ax,%ax
  4011c0:	c3                   	ret
  4011c1:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  4011c8:	00 00 00 00 
  4011cc:	0f 1f 40 00          	nopl   0x0(%rax)

00000000004011d0 <register_tm_clones>:
  4011d0:	be b0 40 40 00       	mov    $0x4040b0,%esi
  4011d5:	48 81 ee b0 40 40 00 	sub    $0x4040b0,%rsi
  4011dc:	48 89 f0             	mov    %rsi,%rax
  4011df:	48 c1 ee 3f          	shr    $0x3f,%rsi
  4011e3:	48 c1 f8 03          	sar    $0x3,%rax
  4011e7:	48 01 c6             	add    %rax,%rsi
  4011ea:	48 d1 fe             	sar    $1,%rsi
  4011ed:	74 11                	je     401200 <register_tm_clones+0x30>
  4011ef:	b8 00 00 00 00       	mov    $0x0,%eax
  4011f4:	48 85 c0             	test   %rax,%rax
  4011f7:	74 07                	je     401200 <register_tm_clones+0x30>
  4011f9:	bf b0 40 40 00       	mov    $0x4040b0,%edi
  4011fe:	ff e0                	jmp    *%rax
  401200:	c3                   	ret
  401201:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  401208:	00 00 00 00 
  40120c:	0f 1f 40 00          	nopl   0x0(%rax)

0000000000401210 <__do_global_dtors_aux>:
  401210:	f3 0f 1e fa          	endbr64
  401214:	80 3d b5 2e 00 00 00 	cmpb   $0x0,0x2eb5(%rip)        # 4040d0 <completed.0>
  40121b:	75 13                	jne    401230 <__do_global_dtors_aux+0x20>
  40121d:	55                   	push   %rbp
  40121e:	48 89 e5             	mov    %rsp,%rbp
  401221:	e8 7a ff ff ff       	call   4011a0 <deregister_tm_clones>
  401226:	c6 05 a3 2e 00 00 01 	movb   $0x1,0x2ea3(%rip)        # 4040d0 <completed.0>
  40122d:	5d                   	pop    %rbp
  40122e:	c3                   	ret
  40122f:	90                   	nop
  401230:	c3                   	ret
  401231:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  401238:	00 00 00 00 
  40123c:	0f 1f 40 00          	nopl   0x0(%rax)

0000000000401240 <frame_dummy>:
  401240:	f3 0f 1e fa          	endbr64
  401244:	eb 8a                	jmp    4011d0 <register_tm_clones>
  401246:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  40124d:	00 00 00 

0000000000401250 <tick>:
  401250:	0f b6 05 5a 2e 00 00 	movzbl 0x2e5a(%rip),%eax        # 4040b1 <__io_pending>
  401257:	c5 fa 10 47 04       	vmovss 0x4(%rdi),%xmm0
  40125c:	c5 fa 10 4f 08       	vmovss 0x8(%rdi),%xmm1
  401261:	c5 fa 10 15 9b 0d 00 	vmovss 0xd9b(%rip),%xmm2        # 402004 <_IO_stdin_used+0x4>
  401268:	00 
  401269:	c5 fa 59 da          	vmulss %xmm2,%xmm0,%xmm3
  40126d:	c5 e2 58 1f          	vaddss (%rdi),%xmm3,%xmm3
  401271:	c5 fa 10 25 8f 0d 00 	vmovss 0xd8f(%rip),%xmm4        # 402008 <_IO_stdin_used+0x8>
  401278:	00 
  401279:	c5 f2 59 ec          	vmulss %xmm4,%xmm1,%xmm5
  40127d:	c5 e2 58 dd          	vaddss %xmm5,%xmm3,%xmm3
  401281:	c5 fa 11 1f          	vmovss %xmm3,(%rdi)
  401285:	c5 e2 58 e9          	vaddss %xmm1,%xmm3,%xmm5
  401289:	c5 d2 59 ea          	vmulss %xmm2,%xmm5,%xmm5
  40128d:	c5 d2 58 c0          	vaddss %xmm0,%xmm5,%xmm0
  401291:	c5 fa 11 47 04       	vmovss %xmm0,0x4(%rdi)
  401296:	c5 e2 59 dc          	vmulss %xmm4,%xmm3,%xmm3
  40129a:	c5 e2 58 c9          	vaddss %xmm1,%xmm3,%xmm1
  40129e:	c4 e3 71 21 4f 0c 10 	vinsertps $0x10,0xc(%rdi),%xmm1,%xmm1
  4012a5:	c5 f0 16 4f 10       	vmovhps 0x10(%rdi),%xmm1,%xmm1
  4012aa:	c5 fa 59 c2          	vmulss %xmm2,%xmm0,%xmm0
  4012ae:	c4 e3 79 0c 05 68 0d 	vblendps $0xe,0xd68(%rip),%xmm0,%xmm0        # 402020 <_IO_stdin_used+0x20>
  4012b5:	00 00 0e 
  4012b8:	c5 f0 58 c0          	vaddps %xmm0,%xmm1,%xmm0
  4012bc:	c5 f8 11 47 08       	vmovups %xmm0,0x8(%rdi)
  4012c1:	48 ff 47 18          	incq   0x18(%rdi)
  4012c5:	c3                   	ret
  4012c6:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  4012cd:	00 00 00 

00000000004012d0 <init_state>:
  4012d0:	53                   	push   %rbx
  4012d1:	48 89 fb             	mov    %rdi,%rbx
  4012d4:	c5 fb 10 05 54 0d 00 	vmovsd 0xd54(%rip),%xmm0        # 402030 <_IO_stdin_used+0x30>
  4012db:	00 
  4012dc:	c5 fb 11 07          	vmovsd %xmm0,(%rdi)
  4012e0:	c7 47 08 cd cc 4c 3e 	movl   $0x3e4ccccd,0x8(%rdi)
  4012e7:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4012eb:	c5 f8 11 47 0c       	vmovups %xmm0,0xc(%rdi)
  4012f0:	c7 47 1c 00 00 00 00 	movl   $0x0,0x1c(%rdi)
  4012f7:	bf d8 20 40 00       	mov    $0x4020d8,%edi
  4012fc:	e8 ff 03 00 00       	call   401700 <__get_env_int>
  401301:	48 89 43 20          	mov    %rax,0x20(%rbx)
  401305:	5b                   	pop    %rbx
  401306:	c3                   	ret
  401307:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  40130e:	00 00 

0000000000401310 <reactor_tick>:
  401310:	0f b6 05 9a 2d 00 00 	movzbl 0x2d9a(%rip),%eax        # 4040b1 <__io_pending>
  401317:	0f b6 0d 93 2d 00 00 	movzbl 0x2d93(%rip),%ecx        # 4040b1 <__io_pending>
  40131e:	48 8b 47 18          	mov    0x18(%rdi),%rax
  401322:	48 3b 47 20          	cmp    0x20(%rdi),%rax
  401326:	7d 7d                	jge    4013a5 <reactor_tick+0x95>
  401328:	80 e1 01             	and    $0x1,%cl
  40132b:	74 78                	je     4013a5 <reactor_tick+0x95>
  40132d:	0f b6 0d 7d 2d 00 00 	movzbl 0x2d7d(%rip),%ecx        # 4040b1 <__io_pending>
  401334:	c5 fa 10 47 04       	vmovss 0x4(%rdi),%xmm0
  401339:	c5 fa 10 4f 08       	vmovss 0x8(%rdi),%xmm1
  40133e:	c5 fa 10 15 be 0c 00 	vmovss 0xcbe(%rip),%xmm2        # 402004 <_IO_stdin_used+0x4>
  401345:	00 
  401346:	c5 fa 59 da          	vmulss %xmm2,%xmm0,%xmm3
  40134a:	c5 e2 58 1f          	vaddss (%rdi),%xmm3,%xmm3
  40134e:	c5 fa 10 25 b2 0c 00 	vmovss 0xcb2(%rip),%xmm4        # 402008 <_IO_stdin_used+0x8>
  401355:	00 
  401356:	c5 f2 59 ec          	vmulss %xmm4,%xmm1,%xmm5
  40135a:	c5 e2 58 dd          	vaddss %xmm5,%xmm3,%xmm3
  40135e:	c5 fa 11 1f          	vmovss %xmm3,(%rdi)
  401362:	c5 e2 58 e9          	vaddss %xmm1,%xmm3,%xmm5
  401366:	c5 d2 59 ea          	vmulss %xmm2,%xmm5,%xmm5
  40136a:	c5 d2 58 c0          	vaddss %xmm0,%xmm5,%xmm0
  40136e:	c5 fa 11 47 04       	vmovss %xmm0,0x4(%rdi)
  401373:	c5 e2 59 dc          	vmulss %xmm4,%xmm3,%xmm3
  401377:	c5 fa 59 c2          	vmulss %xmm2,%xmm0,%xmm0
  40137b:	c5 e2 58 c9          	vaddss %xmm1,%xmm3,%xmm1
  40137f:	c4 e3 71 21 4f 0c 10 	vinsertps $0x10,0xc(%rdi),%xmm1,%xmm1
  401386:	c5 f0 16 4f 10       	vmovhps 0x10(%rdi),%xmm1,%xmm1
  40138b:	c4 e3 79 0c 05 8b 0c 	vblendps $0xe,0xc8b(%rip),%xmm0,%xmm0        # 402020 <_IO_stdin_used+0x20>
  401392:	00 00 0e 
  401395:	c5 f0 58 c0          	vaddps %xmm0,%xmm1,%xmm0
  401399:	c5 f8 11 47 08       	vmovups %xmm0,0x8(%rdi)
  40139e:	48 ff c0             	inc    %rax
  4013a1:	48 89 47 18          	mov    %rax,0x18(%rdi)
  4013a5:	c3                   	ret
  4013a6:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  4013ad:	00 00 00 

00000000004013b0 <main>:
  4013b0:	41 56                	push   %r14
  4013b2:	53                   	push   %rbx
  4013b3:	50                   	push   %rax
  4013b4:	bf d8 20 40 00       	mov    $0x4020d8,%edi
  4013b9:	e8 42 03 00 00       	call   401700 <__get_env_int>
  4013be:	48 89 c3             	mov    %rax,%rbx
  4013c1:	e8 7a 01 00 00       	call   401540 <__rt_init>
  4013c6:	e8 15 05 00 00       	call   4018e0 <__rt_poll>
  4013cb:	4c 8d 73 fd          	lea    -0x3(%rbx),%r14
  4013cf:	c5 fa 10 35 2d 0c 00 	vmovss 0xc2d(%rip),%xmm6        # 402004 <_IO_stdin_used+0x4>
  4013d6:	00 
  4013d7:	c5 fa 10 3d 29 0c 00 	vmovss 0xc29(%rip),%xmm7        # 402008 <_IO_stdin_used+0x8>
  4013de:	00 
  4013df:	eb 24                	jmp    401405 <main+0x55>
  4013e1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4013e8:	0f 1f 84 00 00 00 00 
  4013ef:	00 
  4013f0:	e8 5b 03 00 00       	call   401750 <__rt_wait>
  4013f5:	c5 fa 10 3d 0b 0c 00 	vmovss 0xc0b(%rip),%xmm7        # 402008 <_IO_stdin_used+0x8>
  4013fc:	00 
  4013fd:	c5 fa 10 35 ff 0b 00 	vmovss 0xbff(%rip),%xmm6        # 402004 <_IO_stdin_used+0x4>
  401404:	00 
  401405:	0f b6 05 a5 2c 00 00 	movzbl 0x2ca5(%rip),%eax        # 4040b1 <__io_pending>
  40140c:	31 c0                	xor    %eax,%eax
  40140e:	c5 fa 10 0d fe 0b 00 	vmovss 0xbfe(%rip),%xmm1        # 402014 <_IO_stdin_used+0x14>
  401415:	00 
  401416:	c5 f9 6e 05 f2 0b 00 	vmovd  0xbf2(%rip),%xmm0        # 402010 <_IO_stdin_used+0x10>
  40141d:	00 
  40141e:	c5 fa 10 15 e6 0b 00 	vmovss 0xbe6(%rip),%xmm2        # 40200c <_IO_stdin_used+0xc>
  401425:	00 
  401426:	e9 b1 00 00 00       	jmp    4014dc <main+0x12c>
  40142b:	0f 1f 44 00 00       	nopl   0x0(%rax,%rax,1)
  401430:	c5 fa 59 de          	vmulss %xmm6,%xmm0,%xmm3
  401434:	c5 e2 58 e2          	vaddss %xmm2,%xmm3,%xmm4
  401438:	c5 f2 59 ef          	vmulss %xmm7,%xmm1,%xmm5
  40143c:	c5 da 58 e5          	vaddss %xmm5,%xmm4,%xmm4
  401440:	c5 f2 58 ea          	vaddss %xmm2,%xmm1,%xmm5
  401444:	c5 d2 59 ee          	vmulss %xmm6,%xmm5,%xmm5
  401448:	c5 d2 58 c0          	vaddss %xmm0,%xmm5,%xmm0
  40144c:	c5 ea 59 d7          	vmulss %xmm7,%xmm2,%xmm2
  401450:	c5 e2 58 d2          	vaddss %xmm2,%xmm3,%xmm2
  401454:	c5 ea 58 c9          	vaddss %xmm1,%xmm2,%xmm1
  401458:	c5 fa 59 d6          	vmulss %xmm6,%xmm0,%xmm2
  40145c:	c5 f2 59 df          	vmulss %xmm7,%xmm1,%xmm3
  401460:	c5 da 58 ea          	vaddss %xmm2,%xmm4,%xmm5
  401464:	c5 e2 58 dd          	vaddss %xmm5,%xmm3,%xmm3
  401468:	c5 f2 58 ec          	vaddss %xmm4,%xmm1,%xmm5
  40146c:	c5 d2 59 ee          	vmulss %xmm6,%xmm5,%xmm5
  401470:	c5 d2 58 c0          	vaddss %xmm0,%xmm5,%xmm0
  401474:	c5 da 59 e7          	vmulss %xmm7,%xmm4,%xmm4
  401478:	c5 f2 58 ca          	vaddss %xmm2,%xmm1,%xmm1
  40147c:	c5 da 58 c9          	vaddss %xmm1,%xmm4,%xmm1
  401480:	c5 fa 59 e6          	vmulss %xmm6,%xmm0,%xmm4
  401484:	c5 f2 59 d7          	vmulss %xmm7,%xmm1,%xmm2
  401488:	c5 e2 58 d2          	vaddss %xmm2,%xmm3,%xmm2
  40148c:	c5 ea 58 d4          	vaddss %xmm4,%xmm2,%xmm2
  401490:	c5 f2 58 eb          	vaddss %xmm3,%xmm1,%xmm5
  401494:	c5 d2 59 ee          	vmulss %xmm6,%xmm5,%xmm5
  401498:	c5 d2 58 c0          	vaddss %xmm0,%xmm5,%xmm0
  40149c:	c5 e2 59 df          	vmulss %xmm7,%xmm3,%xmm3
  4014a0:	c5 f2 58 cb          	vaddss %xmm3,%xmm1,%xmm1
  4014a4:	c5 f2 58 e4          	vaddss %xmm4,%xmm1,%xmm4
  4014a8:	c5 fa 59 ce          	vmulss %xmm6,%xmm0,%xmm1
  4014ac:	c5 f2 58 da          	vaddss %xmm2,%xmm1,%xmm3
  4014b0:	c5 da 59 ef          	vmulss %xmm7,%xmm4,%xmm5
  4014b4:	c5 e2 58 dd          	vaddss %xmm5,%xmm3,%xmm3
  4014b8:	c5 da 58 ea          	vaddss %xmm2,%xmm4,%xmm5
  4014bc:	b9 04 00 00 00       	mov    $0x4,%ecx
  4014c1:	c5 da 58 c9          	vaddss %xmm1,%xmm4,%xmm1
  4014c5:	c5 d2 59 e6          	vmulss %xmm6,%xmm5,%xmm4
  4014c9:	c5 da 58 c0          	vaddss %xmm0,%xmm4,%xmm0
  4014cd:	c5 ea 59 d7          	vmulss %xmm7,%xmm2,%xmm2
  4014d1:	c5 f2 58 ca          	vaddss %xmm2,%xmm1,%xmm1
  4014d5:	48 01 c8             	add    %rcx,%rax
  4014d8:	c5 f8 28 d3          	vmovaps %xmm3,%xmm2
  4014dc:	4c 39 f0             	cmp    %r14,%rax
  4014df:	0f 8c 4b ff ff ff    	jl     401430 <main+0x80>
  4014e5:	48 39 d8             	cmp    %rbx,%rax
  4014e8:	7d 26                	jge    401510 <main+0x160>
  4014ea:	c5 fa 59 e6          	vmulss %xmm6,%xmm0,%xmm4
  4014ee:	c5 da 58 da          	vaddss %xmm2,%xmm4,%xmm3
  4014f2:	c5 f2 59 ef          	vmulss %xmm7,%xmm1,%xmm5
  4014f6:	c5 e2 58 dd          	vaddss %xmm5,%xmm3,%xmm3
  4014fa:	c5 f2 58 ea          	vaddss %xmm2,%xmm1,%xmm5
  4014fe:	b9 01 00 00 00       	mov    $0x1,%ecx
  401503:	eb bc                	jmp    4014c1 <main+0x111>
  401505:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  40150c:	00 00 00 00 
  401510:	c5 f9 7e c1          	vmovd  %xmm0,%ecx
  401514:	85 c9                	test   %ecx,%ecx
  401516:	0f 88 d4 fe ff ff    	js     4013f0 <main+0x40>
  40151c:	48 39 d8             	cmp    %rbx,%rax
  40151f:	0f 85 cb fe ff ff    	jne    4013f0 <main+0x40>
  401525:	31 c0                	xor    %eax,%eax
  401527:	48 83 c4 08          	add    $0x8,%rsp
  40152b:	5b                   	pop    %rbx
  40152c:	41 5e                	pop    %r14
  40152e:	c3                   	ret
  40152f:	90                   	nop

0000000000401530 <brief_rt_ctor>:
  401530:	e9 0b 00 00 00       	jmp    401540 <__rt_init>
  401535:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  40153c:	00 00 00 00 

0000000000401540 <__rt_init>:
  401540:	53                   	push   %rbx
  401541:	48 81 ec 00 01 00 00 	sub    $0x100,%rsp
  401548:	be c0 16 40 00       	mov    $0x4016c0,%esi
  40154d:	bf 02 00 00 00       	mov    $0x2,%edi
  401552:	e8 59 fb ff ff       	call   4010b0 <signal@plt>
  401557:	be d0 16 40 00       	mov    $0x4016d0,%esi
  40155c:	bf 0f 00 00 00       	mov    $0xf,%edi
  401561:	e8 4a fb ff ff       	call   4010b0 <signal@plt>
  401566:	be e0 16 40 00       	mov    $0x4016e0,%esi
  40156b:	bf 01 00 00 00       	mov    $0x1,%edi
  401570:	e8 3b fb ff ff       	call   4010b0 <signal@plt>
  401575:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  401579:	c5 fc 11 84 24 e0 00 	vmovups %ymm0,0xe0(%rsp)
  401580:	00 00 
  401582:	c5 fc 11 84 24 d0 00 	vmovups %ymm0,0xd0(%rsp)
  401589:	00 00 
  40158b:	c5 fc 11 84 24 b0 00 	vmovups %ymm0,0xb0(%rsp)
  401592:	00 00 
  401594:	c5 fc 11 84 24 90 00 	vmovups %ymm0,0x90(%rsp)
  40159b:	00 00 
  40159d:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  4015a3:	48 c7 44 24 68 f0 16 	movq   $0x4016f0,0x68(%rsp)
  4015aa:	40 00 
  4015ac:	c7 84 24 f0 00 00 00 	movl   $0x4,0xf0(%rsp)
  4015b3:	04 00 00 00 
  4015b7:	c5 f8 77             	vzeroupper
  4015ba:	e8 61 fb ff ff       	call   401120 <__libc_current_sigrtmin@plt>
  4015bf:	8d 78 01             	lea    0x1(%rax),%edi
  4015c2:	48 8d 5c 24 68       	lea    0x68(%rsp),%rbx
  4015c7:	48 89 de             	mov    %rbx,%rsi
  4015ca:	31 d2                	xor    %edx,%edx
  4015cc:	e8 7f fa ff ff       	call   401050 <sigaction@plt>
  4015d1:	e8 4a fb ff ff       	call   401120 <__libc_current_sigrtmin@plt>
  4015d6:	8d 78 02             	lea    0x2(%rax),%edi
  4015d9:	48 89 de             	mov    %rbx,%rsi
  4015dc:	31 d2                	xor    %edx,%edx
  4015de:	e8 6d fa ff ff       	call   401050 <sigaction@plt>
  4015e3:	e8 38 fb ff ff       	call   401120 <__libc_current_sigrtmin@plt>
  4015e8:	ff c0                	inc    %eax
  4015ea:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4015ee:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
  4015f3:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
  4015f9:	89 44 24 08          	mov    %eax,0x8(%rsp)
  4015fd:	48 89 e6             	mov    %rsp,%rsi
  401600:	ba d8 40 40 00       	mov    $0x4040d8,%edx
  401605:	31 ff                	xor    %edi,%edi
  401607:	c5 f8 77             	vzeroupper
  40160a:	e8 61 fa ff ff       	call   401070 <timer_create@plt>
  40160f:	85 c0                	test   %eax,%eax
  401611:	75 27                	jne    40163a <__rt_init+0xfa>
  401613:	c4 e2 7d 1a 05 24 0a 	vbroadcastf128 0xa24(%rip),%ymm0        # 402040 <_IO_stdin_used+0x40>
  40161a:	00 00 
  40161c:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
  401622:	48 8b 3d af 2a 00 00 	mov    0x2aaf(%rip),%rdi        # 4040d8 <g_timer_1hz>
  401629:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
  40162e:	31 f6                	xor    %esi,%esi
  401630:	31 c9                	xor    %ecx,%ecx
  401632:	c5 f8 77             	vzeroupper
  401635:	e8 06 fa ff ff       	call   401040 <timer_settime@plt>
  40163a:	e8 e1 fa ff ff       	call   401120 <__libc_current_sigrtmin@plt>
  40163f:	83 c0 02             	add    $0x2,%eax
  401642:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  401646:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
  40164b:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
  401651:	89 44 24 08          	mov    %eax,0x8(%rsp)
  401655:	48 89 e6             	mov    %rsp,%rsi
  401658:	ba e0 40 40 00       	mov    $0x4040e0,%edx
  40165d:	31 ff                	xor    %edi,%edi
  40165f:	c5 f8 77             	vzeroupper
  401662:	e8 09 fa ff ff       	call   401070 <timer_create@plt>
  401667:	85 c0                	test   %eax,%eax
  401669:	75 27                	jne    401692 <__rt_init+0x152>
  40166b:	c4 e2 7d 1a 05 dc 09 	vbroadcastf128 0x9dc(%rip),%ymm0        # 402050 <_IO_stdin_used+0x50>
  401672:	00 00 
  401674:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
  40167a:	48 8b 3d 5f 2a 00 00 	mov    0x2a5f(%rip),%rdi        # 4040e0 <g_timer_100hz>
  401681:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
  401686:	31 f6                	xor    %esi,%esi
  401688:	31 c9                	xor    %ecx,%ecx
  40168a:	c5 f8 77             	vzeroupper
  40168d:	e8 ae f9 ff ff       	call   401040 <timer_settime@plt>
  401692:	48 8b 05 2f 29 00 00 	mov    0x292f(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401699:	48 8b 38             	mov    (%rax),%rdi
  40169c:	31 f6                	xor    %esi,%esi
  40169e:	ba 01 00 00 00       	mov    $0x1,%edx
  4016a3:	31 c9                	xor    %ecx,%ecx
  4016a5:	e8 66 fa ff ff       	call   401110 <setvbuf@plt>
  4016aa:	c6 05 00 2a 00 00 01 	movb   $0x1,0x2a00(%rip)        # 4040b1 <__io_pending>
  4016b1:	48 81 c4 00 01 00 00 	add    $0x100,%rsp
  4016b8:	5b                   	pop    %rbx
  4016b9:	c3                   	ret
  4016ba:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

00000000004016c0 <handle_sigint>:
  4016c0:	c6 05 eb 29 00 00 01 	movb   $0x1,0x29eb(%rip)        # 4040b2 <__sigint_flag>
  4016c7:	c6 05 e3 29 00 00 01 	movb   $0x1,0x29e3(%rip)        # 4040b1 <__io_pending>
  4016ce:	c3                   	ret
  4016cf:	90                   	nop

00000000004016d0 <handle_sigterm>:
  4016d0:	c6 05 dc 29 00 00 01 	movb   $0x1,0x29dc(%rip)        # 4040b3 <__sigterm_flag>
  4016d7:	c6 05 d3 29 00 00 01 	movb   $0x1,0x29d3(%rip)        # 4040b1 <__io_pending>
  4016de:	c3                   	ret
  4016df:	90                   	nop

00000000004016e0 <handle_sighup>:
  4016e0:	c6 05 cd 29 00 00 01 	movb   $0x1,0x29cd(%rip)        # 4040b4 <__sighup_flag>
  4016e7:	c6 05 c3 29 00 00 01 	movb   $0x1,0x29c3(%rip)        # 4040b1 <__io_pending>
  4016ee:	c3                   	ret
  4016ef:	90                   	nop

00000000004016f0 <handle_timer>:
  4016f0:	48 ff 05 c1 29 00 00 	incq   0x29c1(%rip)        # 4040b8 <__timer_1hz>
  4016f7:	c6 05 b3 29 00 00 01 	movb   $0x1,0x29b3(%rip)        # 4040b1 <__io_pending>
  4016fe:	c3                   	ret
  4016ff:	90                   	nop

0000000000401700 <__get_env_int>:
  401700:	53                   	push   %rbx
  401701:	48 83 ec 10          	sub    $0x10,%rsp
  401705:	e8 26 f9 ff ff       	call   401030 <getenv@plt>
  40170a:	48 85 c0             	test   %rax,%rax
  40170d:	74 32                	je     401741 <__get_env_int+0x41>
  40170f:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
  401716:	00 00 
  401718:	48 8d 74 24 08       	lea    0x8(%rsp),%rsi
  40171d:	48 89 c7             	mov    %rax,%rdi
  401720:	ba 0a 00 00 00       	mov    $0xa,%edx
  401725:	48 89 c3             	mov    %rax,%rbx
  401728:	e8 a3 f9 ff ff       	call   4010d0 <strtol@plt>
  40172d:	48 89 c1             	mov    %rax,%rcx
  401730:	31 c0                	xor    %eax,%eax
  401732:	48 39 5c 24 08       	cmp    %rbx,0x8(%rsp)
  401737:	48 0f 45 c1          	cmovne %rcx,%rax
  40173b:	48 83 c4 10          	add    $0x10,%rsp
  40173f:	5b                   	pop    %rbx
  401740:	c3                   	ret
  401741:	31 c0                	xor    %eax,%eax
  401743:	48 83 c4 10          	add    $0x10,%rsp
  401747:	5b                   	pop    %rbx
  401748:	c3                   	ret
  401749:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000401750 <__rt_wait>:
  401750:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
  401757:	8b 3d 4b 29 00 00    	mov    0x294b(%rip),%edi        # 4040a8 <g_epoll_fd>
  40175d:	85 ff                	test   %edi,%edi
  40175f:	79 3f                	jns    4017a0 <__rt_wait+0x50>
  401761:	31 ff                	xor    %edi,%edi
  401763:	e8 e8 f9 ff ff       	call   401150 <epoll_create1@plt>
  401768:	89 05 3a 29 00 00    	mov    %eax,0x293a(%rip)        # 4040a8 <g_epoll_fd>
  40176e:	85 c0                	test   %eax,%eax
  401770:	0f 88 d5 00 00 00    	js     40184b <__rt_wait+0xfb>
  401776:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
  40177d:	00 
  40177e:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
  401785:	00 00 
  401787:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
  40178c:	89 c7                	mov    %eax,%edi
  40178e:	be 01 00 00 00       	mov    $0x1,%esi
  401793:	31 d2                	xor    %edx,%edx
  401795:	e8 f6 f8 ff ff       	call   401090 <epoll_ctl@plt>
  40179a:	8b 3d 08 29 00 00    	mov    0x2908(%rip),%edi        # 4040a8 <g_epoll_fd>
  4017a0:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  4017a5:	ba 40 00 00 00       	mov    $0x40,%edx
  4017aa:	b9 64 00 00 00       	mov    $0x64,%ecx
  4017af:	e8 4c f9 ff ff       	call   401100 <epoll_wait@plt>
  4017b4:	85 c0                	test   %eax,%eax
  4017b6:	0f 8e ee 00 00 00    	jle    4018aa <__rt_wait+0x15a>
  4017bc:	89 c1                	mov    %eax,%ecx
  4017be:	83 f8 01             	cmp    $0x1,%eax
  4017c1:	75 1e                	jne    4017e1 <__rt_wait+0x91>
  4017c3:	31 c0                	xor    %eax,%eax
  4017c5:	f6 c1 01             	test   $0x1,%cl
  4017c8:	74 0f                	je     4017d9 <__rt_wait+0x89>
  4017ca:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
  4017ce:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
  4017d3:	0f 84 e0 00 00 00    	je     4018b9 <__rt_wait+0x169>
  4017d9:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  4017e0:	c3                   	ret
  4017e1:	89 c8                	mov    %ecx,%eax
  4017e3:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
  4017e8:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
  4017ed:	48 89 c6             	mov    %rax,%rsi
  4017f0:	eb 18                	jmp    40180a <__rt_wait+0xba>
  4017f2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4017f9:	1f 84 00 00 00 00 00 
  401800:	48 83 c2 18          	add    $0x18,%rdx
  401804:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
  401808:	74 bb                	je     4017c5 <__rt_wait+0x75>
  40180a:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
  40180e:	75 20                	jne    401830 <__rt_wait+0xe0>
  401810:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
  401814:	74 1a                	je     401830 <__rt_wait+0xe0>
  401816:	c6 05 93 28 00 00 01 	movb   $0x1,0x2893(%rip)        # 4040b0 <__stdin_ready>
  40181d:	c6 05 8d 28 00 00 01 	movb   $0x1,0x288d(%rip)        # 4040b1 <__io_pending>
  401824:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
  40182b:	00 00 00 00 00 
  401830:	83 3a 00             	cmpl   $0x0,(%rdx)
  401833:	75 cb                	jne    401800 <__rt_wait+0xb0>
  401835:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
  401839:	74 c5                	je     401800 <__rt_wait+0xb0>
  40183b:	c6 05 6e 28 00 00 01 	movb   $0x1,0x286e(%rip)        # 4040b0 <__stdin_ready>
  401842:	c6 05 68 28 00 00 01 	movb   $0x1,0x2868(%rip)        # 4040b1 <__io_pending>
  401849:	eb b5                	jmp    401800 <__rt_wait+0xb0>
  40184b:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  40184f:	c5 fc 11 44 24 30    	vmovups %ymm0,0x30(%rsp)
  401855:	c5 fc 11 44 24 50    	vmovups %ymm0,0x50(%rsp)
  40185b:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  401861:	c5 f8 28 05 d7 07 00 	vmovaps 0x7d7(%rip),%xmm0        # 402040 <_IO_stdin_used+0x40>
  401868:	00 
  401869:	c5 fc 11 44 24 10    	vmovups %ymm0,0x10(%rsp)
  40186f:	c5 f8 10 05 e9 07 00 	vmovups 0x7e9(%rip),%xmm0        # 402060 <_IO_stdin_used+0x60>
  401876:	00 
  401877:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
  40187c:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  401881:	49 89 e0             	mov    %rsp,%r8
  401884:	bf 01 00 00 00       	mov    $0x1,%edi
  401889:	31 d2                	xor    %edx,%edx
  40188b:	31 c9                	xor    %ecx,%ecx
  40188d:	c5 f8 77             	vzeroupper
  401890:	e8 4b f8 ff ff       	call   4010e0 <select@plt>
  401895:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
  40189a:	74 0e                	je     4018aa <__rt_wait+0x15a>
  40189c:	c6 05 0d 28 00 00 01 	movb   $0x1,0x280d(%rip)        # 4040b0 <__stdin_ready>
  4018a3:	c6 05 07 28 00 00 01 	movb   $0x1,0x2807(%rip)        # 4040b1 <__io_pending>
  4018aa:	c6 05 00 28 00 00 01 	movb   $0x1,0x2800(%rip)        # 4040b1 <__io_pending>
  4018b1:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  4018b8:	c3                   	ret
  4018b9:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
  4018be:	0f 84 15 ff ff ff    	je     4017d9 <__rt_wait+0x89>
  4018c4:	c6 05 e5 27 00 00 01 	movb   $0x1,0x27e5(%rip)        # 4040b0 <__stdin_ready>
  4018cb:	c6 05 df 27 00 00 01 	movb   $0x1,0x27df(%rip)        # 4040b1 <__io_pending>
  4018d2:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  4018d9:	c3                   	ret
  4018da:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

00000000004018e0 <__rt_poll>:
  4018e0:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
  4018e7:	8b 3d bb 27 00 00    	mov    0x27bb(%rip),%edi        # 4040a8 <g_epoll_fd>
  4018ed:	85 ff                	test   %edi,%edi
  4018ef:	79 3f                	jns    401930 <__rt_poll+0x50>
  4018f1:	31 ff                	xor    %edi,%edi
  4018f3:	e8 58 f8 ff ff       	call   401150 <epoll_create1@plt>
  4018f8:	89 05 aa 27 00 00    	mov    %eax,0x27aa(%rip)        # 4040a8 <g_epoll_fd>
  4018fe:	85 c0                	test   %eax,%eax
  401900:	0f 88 d5 00 00 00    	js     4019db <__rt_poll+0xfb>
  401906:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
  40190d:	00 
  40190e:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
  401915:	00 00 
  401917:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
  40191c:	89 c7                	mov    %eax,%edi
  40191e:	be 01 00 00 00       	mov    $0x1,%esi
  401923:	31 d2                	xor    %edx,%edx
  401925:	e8 66 f7 ff ff       	call   401090 <epoll_ctl@plt>
  40192a:	8b 3d 78 27 00 00    	mov    0x2778(%rip),%edi        # 4040a8 <g_epoll_fd>
  401930:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  401935:	ba 40 00 00 00       	mov    $0x40,%edx
  40193a:	31 c9                	xor    %ecx,%ecx
  40193c:	e8 bf f7 ff ff       	call   401100 <epoll_wait@plt>
  401941:	85 c0                	test   %eax,%eax
  401943:	7e 1d                	jle    401962 <__rt_poll+0x82>
  401945:	89 c1                	mov    %eax,%ecx
  401947:	83 f8 01             	cmp    $0x1,%eax
  40194a:	75 25                	jne    401971 <__rt_poll+0x91>
  40194c:	31 c0                	xor    %eax,%eax
  40194e:	f6 c1 01             	test   $0x1,%cl
  401951:	74 0f                	je     401962 <__rt_poll+0x82>
  401953:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
  401957:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
  40195c:	0f 84 cc 00 00 00    	je     401a2e <__rt_poll+0x14e>
  401962:	c6 05 48 27 00 00 01 	movb   $0x1,0x2748(%rip)        # 4040b1 <__io_pending>
  401969:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  401970:	c3                   	ret
  401971:	89 c8                	mov    %ecx,%eax
  401973:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
  401978:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
  40197d:	48 89 c6             	mov    %rax,%rsi
  401980:	eb 18                	jmp    40199a <__rt_poll+0xba>
  401982:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401989:	1f 84 00 00 00 00 00 
  401990:	48 83 c2 18          	add    $0x18,%rdx
  401994:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
  401998:	74 b4                	je     40194e <__rt_poll+0x6e>
  40199a:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
  40199e:	75 20                	jne    4019c0 <__rt_poll+0xe0>
  4019a0:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
  4019a4:	74 1a                	je     4019c0 <__rt_poll+0xe0>
  4019a6:	c6 05 03 27 00 00 01 	movb   $0x1,0x2703(%rip)        # 4040b0 <__stdin_ready>
  4019ad:	c6 05 fd 26 00 00 01 	movb   $0x1,0x26fd(%rip)        # 4040b1 <__io_pending>
  4019b4:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
  4019bb:	00 00 00 00 00 
  4019c0:	83 3a 00             	cmpl   $0x0,(%rdx)
  4019c3:	75 cb                	jne    401990 <__rt_poll+0xb0>
  4019c5:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
  4019c9:	74 c5                	je     401990 <__rt_poll+0xb0>
  4019cb:	c6 05 de 26 00 00 01 	movb   $0x1,0x26de(%rip)        # 4040b0 <__stdin_ready>
  4019d2:	c6 05 d8 26 00 00 01 	movb   $0x1,0x26d8(%rip)        # 4040b1 <__io_pending>
  4019d9:	eb b5                	jmp    401990 <__rt_poll+0xb0>
  4019db:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4019df:	c5 fc 11 44 24 30    	vmovups %ymm0,0x30(%rsp)
  4019e5:	c5 fc 11 44 24 50    	vmovups %ymm0,0x50(%rsp)
  4019eb:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  4019f1:	c5 f8 28 05 47 06 00 	vmovaps 0x647(%rip),%xmm0        # 402040 <_IO_stdin_used+0x40>
  4019f8:	00 
  4019f9:	c5 fc 11 44 24 10    	vmovups %ymm0,0x10(%rsp)
  4019ff:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  401a03:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
  401a08:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  401a0d:	49 89 e0             	mov    %rsp,%r8
  401a10:	bf 01 00 00 00       	mov    $0x1,%edi
  401a15:	31 d2                	xor    %edx,%edx
  401a17:	31 c9                	xor    %ecx,%ecx
  401a19:	c5 f8 77             	vzeroupper
  401a1c:	e8 bf f6 ff ff       	call   4010e0 <select@plt>
  401a21:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
  401a26:	0f 84 36 ff ff ff    	je     401962 <__rt_poll+0x82>
  401a2c:	eb 0b                	jmp    401a39 <__rt_poll+0x159>
  401a2e:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
  401a33:	0f 84 29 ff ff ff    	je     401962 <__rt_poll+0x82>
  401a39:	c6 05 70 26 00 00 01 	movb   $0x1,0x2670(%rip)        # 4040b0 <__stdin_ready>
  401a40:	c6 05 6a 26 00 00 01 	movb   $0x1,0x266a(%rip)        # 4040b1 <__io_pending>
  401a47:	c6 05 63 26 00 00 01 	movb   $0x1,0x2663(%rip)        # 4040b1 <__io_pending>
  401a4e:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  401a55:	c3                   	ret
  401a56:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  401a5d:	00 00 00 

0000000000401a60 <__wait_for_event>:
  401a60:	e9 eb fc ff ff       	jmp    401750 <__rt_wait>
  401a65:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  401a6c:	00 00 00 00 

0000000000401a70 <__print>:
  401a70:	50                   	push   %rax
  401a71:	48 8b 05 50 25 00 00 	mov    0x2550(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401a78:	48 8b 30             	mov    (%rax),%rsi
  401a7b:	e8 00 f6 ff ff       	call   401080 <fputs@plt>
  401a80:	b8 01 00 00 00       	mov    $0x1,%eax
  401a85:	59                   	pop    %rcx
  401a86:	c3                   	ret
  401a87:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  401a8e:	00 00 

0000000000401a90 <__print_int>:
  401a90:	50                   	push   %rax
  401a91:	48 89 fa             	mov    %rdi,%rdx
  401a94:	48 8b 05 45 25 00 00 	mov    0x2545(%rip),%rax        # 403fe0 <stderr@GLIBC_2.2.5>
  401a9b:	48 8b 38             	mov    (%rax),%rdi
  401a9e:	be de 20 40 00       	mov    $0x4020de,%esi
  401aa3:	31 c0                	xor    %eax,%eax
  401aa5:	e8 16 f6 ff ff       	call   4010c0 <fprintf@plt>
  401aaa:	b8 01 00 00 00       	mov    $0x1,%eax
  401aaf:	59                   	pop    %rcx
  401ab0:	c3                   	ret
  401ab1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401ab8:	0f 1f 84 00 00 00 00 
  401abf:	00 

0000000000401ac0 <__print_float>:
  401ac0:	50                   	push   %rax
  401ac1:	48 8b 05 18 25 00 00 	mov    0x2518(%rip),%rax        # 403fe0 <stderr@GLIBC_2.2.5>
  401ac8:	48 8b 38             	mov    (%rax),%rdi
  401acb:	c5 fa 5a c0          	vcvtss2sd %xmm0,%xmm0,%xmm0
  401acf:	be e4 20 40 00       	mov    $0x4020e4,%esi
  401ad4:	b0 01                	mov    $0x1,%al
  401ad6:	e8 e5 f5 ff ff       	call   4010c0 <fprintf@plt>
  401adb:	b8 01 00 00 00       	mov    $0x1,%eax
  401ae0:	59                   	pop    %rcx
  401ae1:	c3                   	ret
  401ae2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401ae9:	1f 84 00 00 00 00 00 

0000000000401af0 <__sqrtf>:
  401af0:	e9 fb f5 ff ff       	jmp    4010f0 <sqrtf@plt>
  401af5:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  401afc:	00 00 00 00 

0000000000401b00 <__exit>:
  401b00:	50                   	push   %rax
  401b01:	31 ff                	xor    %edi,%edi
  401b03:	e8 28 f6 ff ff       	call   401130 <exit@plt>
  401b08:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  401b0f:	00 

0000000000401b10 <__print_str_len>:
  401b10:	53                   	push   %rbx
  401b11:	48 89 f3             	mov    %rsi,%rbx
  401b14:	48 8b 05 ad 24 00 00 	mov    0x24ad(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401b1b:	48 8b 08             	mov    (%rax),%rcx
  401b1e:	be 01 00 00 00       	mov    $0x1,%esi
  401b23:	48 89 da             	mov    %rbx,%rdx
  401b26:	e8 15 f6 ff ff       	call   401140 <fwrite@plt>
  401b2b:	48 89 d8             	mov    %rbx,%rax
  401b2e:	5b                   	pop    %rbx
  401b2f:	c3                   	ret

0000000000401b30 <__write_bytes>:
  401b30:	53                   	push   %rbx
  401b31:	48 89 f3             	mov    %rsi,%rbx
  401b34:	48 8b 05 8d 24 00 00 	mov    0x248d(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401b3b:	48 8b 08             	mov    (%rax),%rcx
  401b3e:	be 01 00 00 00       	mov    $0x1,%esi
  401b43:	48 89 da             	mov    %rbx,%rdx
  401b46:	e8 f5 f5 ff ff       	call   401140 <fwrite@plt>
  401b4b:	48 89 d8             	mov    %rbx,%rax
  401b4e:	5b                   	pop    %rbx
  401b4f:	c3                   	ret

0000000000401b50 <__read_stdin>:
  401b50:	48 89 f2             	mov    %rsi,%rdx
  401b53:	48 8b 05 76 24 00 00 	mov    0x2476(%rip),%rax        # 403fd0 <stdin@GLIBC_2.2.5>
  401b5a:	48 8b 08             	mov    (%rax),%rcx
  401b5d:	be 01 00 00 00       	mov    $0x1,%esi
  401b62:	e9 f9 f4 ff ff       	jmp    401060 <fread@plt>
  401b67:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  401b6e:	00 00 

0000000000401b70 <__putchar>:
  401b70:	53                   	push   %rbx
  401b71:	48 89 fb             	mov    %rdi,%rbx
  401b74:	48 8b 05 4d 24 00 00 	mov    0x244d(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401b7b:	48 8b 30             	mov    (%rax),%rsi
  401b7e:	e8 1d f5 ff ff       	call   4010a0 <putc@plt>
  401b83:	48 89 d8             	mov    %rbx,%rax
  401b86:	5b                   	pop    %rbx
  401b87:	c3                   	ret
  401b88:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  401b8f:	00 

0000000000401b90 <brief_thread_pool_init>:
  401b90:	c3                   	ret
  401b91:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401b98:	0f 1f 84 00 00 00 00 
  401b9f:	00 

0000000000401ba0 <brief_barrier_release>:
  401ba0:	c3                   	ret
  401ba1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401ba8:	0f 1f 84 00 00 00 00 
  401baf:	00 

0000000000401bb0 <brief_barrier_wait>:
  401bb0:	c3                   	ret
  401bb1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401bb8:	0f 1f 84 00 00 00 00 
  401bbf:	00 

0000000000401bc0 <brief_thread_pool_shutdown>:
  401bc0:	c3                   	ret

Disassembly of section .fini:

0000000000401bc4 <_fini>:
  401bc4:	f3 0f 1e fa          	endbr64
  401bc8:	48 83 ec 08          	sub    $0x8,%rsp
  401bcc:	48 83 c4 08          	add    $0x8,%rsp
  401bd0:	c3                   	ret
