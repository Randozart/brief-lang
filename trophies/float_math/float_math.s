
benchmarks/float_math:     file format elf64-x86-64


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
  401178:	48 c7 c7 60 13 40 00 	mov    $0x401360,%rdi
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
  401257:	c5 fb 10 47 04       	vmovsd 0x4(%rdi),%xmm0
  40125c:	c5 f8 59 0d ac 0d 00 	vmulps 0xdac(%rip),%xmm0,%xmm1        # 402010 <_IO_stdin_used+0x10>
  401263:	00 
  401264:	c5 fa 10 17          	vmovss (%rdi),%xmm2
  401268:	c4 e3 69 21 c0 1c    	vinsertps $0x1c,%xmm0,%xmm2,%xmm0
  40126e:	c5 f0 58 c0          	vaddps %xmm0,%xmm1,%xmm0
  401272:	c5 f8 13 07          	vmovlps %xmm0,(%rdi)
  401276:	c5 fa 10 05 86 0d 00 	vmovss 0xd86(%rip),%xmm0        # 402004 <_IO_stdin_used+0x4>
  40127d:	00 
  40127e:	c5 fa 58 4f 0c       	vaddss 0xc(%rdi),%xmm0,%xmm1
  401283:	c5 fa 11 4f 0c       	vmovss %xmm1,0xc(%rdi)
  401288:	c5 fa 58 4f 1c       	vaddss 0x1c(%rdi),%xmm0,%xmm1
  40128d:	c5 fa 11 4f 1c       	vmovss %xmm1,0x1c(%rdi)
  401292:	c5 fa 58 47 2c       	vaddss 0x2c(%rdi),%xmm0,%xmm0
  401297:	c5 fa 11 47 2c       	vmovss %xmm0,0x2c(%rdi)
  40129c:	48 ff 47 30          	incq   0x30(%rdi)
  4012a0:	c3                   	ret
  4012a1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4012a8:	0f 1f 84 00 00 00 00 
  4012af:	00 

00000000004012b0 <init_state>:
  4012b0:	53                   	push   %rbx
  4012b1:	48 89 fb             	mov    %rdi,%rbx
  4012b4:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4012b8:	c5 f8 11 47 10       	vmovups %xmm0,0x10(%rdi)
  4012bd:	c5 f8 11 07          	vmovups %xmm0,(%rdi)
  4012c1:	c5 f8 11 47 20       	vmovups %xmm0,0x20(%rdi)
  4012c6:	48 c7 47 30 00 00 00 	movq   $0x0,0x30(%rdi)
  4012cd:	00 
  4012ce:	bf b8 20 40 00       	mov    $0x4020b8,%edi
  4012d3:	e8 b8 02 00 00       	call   401590 <__get_env_int>
  4012d8:	48 89 43 38          	mov    %rax,0x38(%rbx)
  4012dc:	5b                   	pop    %rbx
  4012dd:	c3                   	ret
  4012de:	66 90                	xchg   %ax,%ax

00000000004012e0 <reactor_tick>:
  4012e0:	0f b6 05 ca 2d 00 00 	movzbl 0x2dca(%rip),%eax        # 4040b1 <__io_pending>
  4012e7:	0f b6 0d c3 2d 00 00 	movzbl 0x2dc3(%rip),%ecx        # 4040b1 <__io_pending>
  4012ee:	48 8b 47 30          	mov    0x30(%rdi),%rax
  4012f2:	48 3b 47 38          	cmp    0x38(%rdi),%rax
  4012f6:	7d 58                	jge    401350 <reactor_tick+0x70>
  4012f8:	80 e1 01             	and    $0x1,%cl
  4012fb:	74 53                	je     401350 <reactor_tick+0x70>
  4012fd:	0f b6 0d ad 2d 00 00 	movzbl 0x2dad(%rip),%ecx        # 4040b1 <__io_pending>
  401304:	c5 fb 10 47 04       	vmovsd 0x4(%rdi),%xmm0
  401309:	c5 f8 59 0d ff 0c 00 	vmulps 0xcff(%rip),%xmm0,%xmm1        # 402010 <_IO_stdin_used+0x10>
  401310:	00 
  401311:	c5 fa 10 17          	vmovss (%rdi),%xmm2
  401315:	c4 e3 69 21 c0 1c    	vinsertps $0x1c,%xmm0,%xmm2,%xmm0
  40131b:	c5 f0 58 c0          	vaddps %xmm0,%xmm1,%xmm0
  40131f:	c5 f8 13 07          	vmovlps %xmm0,(%rdi)
  401323:	c5 fa 10 05 d9 0c 00 	vmovss 0xcd9(%rip),%xmm0        # 402004 <_IO_stdin_used+0x4>
  40132a:	00 
  40132b:	c5 fa 58 4f 0c       	vaddss 0xc(%rdi),%xmm0,%xmm1
  401330:	c5 fa 11 4f 0c       	vmovss %xmm1,0xc(%rdi)
  401335:	c5 fa 58 4f 1c       	vaddss 0x1c(%rdi),%xmm0,%xmm1
  40133a:	c5 fa 11 4f 1c       	vmovss %xmm1,0x1c(%rdi)
  40133f:	c5 fa 58 47 2c       	vaddss 0x2c(%rdi),%xmm0,%xmm0
  401344:	c5 fa 11 47 2c       	vmovss %xmm0,0x2c(%rdi)
  401349:	48 ff c0             	inc    %rax
  40134c:	48 89 47 30          	mov    %rax,0x30(%rdi)
  401350:	c3                   	ret
  401351:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401358:	0f 1f 84 00 00 00 00 
  40135f:	00 

0000000000401360 <main>:
  401360:	41 56                	push   %r14
  401362:	53                   	push   %rbx
  401363:	50                   	push   %rax
  401364:	bf b8 20 40 00       	mov    $0x4020b8,%edi
  401369:	e8 22 02 00 00       	call   401590 <__get_env_int>
  40136e:	48 89 c3             	mov    %rax,%rbx
  401371:	e8 5a 00 00 00       	call   4013d0 <__rt_init>
  401376:	e8 f5 03 00 00       	call   401770 <__rt_poll>
  40137b:	4c 8d 73 fd          	lea    -0x3(%rbx),%r14
  40137f:	90                   	nop
  401380:	0f b6 05 2a 2d 00 00 	movzbl 0x2d2a(%rip),%eax        # 4040b1 <__io_pending>
  401387:	31 c0                	xor    %eax,%eax
  401389:	eb 08                	jmp    401393 <main+0x33>
  40138b:	0f 1f 44 00 00       	nopl   0x0(%rax,%rax,1)
  401390:	48 01 c8             	add    %rcx,%rax
  401393:	b9 04 00 00 00       	mov    $0x4,%ecx
  401398:	4c 39 f0             	cmp    %r14,%rax
  40139b:	7c f3                	jl     401390 <main+0x30>
  40139d:	b9 01 00 00 00       	mov    $0x1,%ecx
  4013a2:	48 39 d8             	cmp    %rbx,%rax
  4013a5:	7c e9                	jl     401390 <main+0x30>
  4013a7:	74 07                	je     4013b0 <main+0x50>
  4013a9:	e8 32 02 00 00       	call   4015e0 <__rt_wait>
  4013ae:	eb d0                	jmp    401380 <main+0x20>
  4013b0:	31 c0                	xor    %eax,%eax
  4013b2:	48 83 c4 08          	add    $0x8,%rsp
  4013b6:	5b                   	pop    %rbx
  4013b7:	41 5e                	pop    %r14
  4013b9:	c3                   	ret
  4013ba:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

00000000004013c0 <brief_rt_ctor>:
  4013c0:	e9 0b 00 00 00       	jmp    4013d0 <__rt_init>
  4013c5:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  4013cc:	00 00 00 00 

00000000004013d0 <__rt_init>:
  4013d0:	53                   	push   %rbx
  4013d1:	48 81 ec 00 01 00 00 	sub    $0x100,%rsp
  4013d8:	be 50 15 40 00       	mov    $0x401550,%esi
  4013dd:	bf 02 00 00 00       	mov    $0x2,%edi
  4013e2:	e8 c9 fc ff ff       	call   4010b0 <signal@plt>
  4013e7:	be 60 15 40 00       	mov    $0x401560,%esi
  4013ec:	bf 0f 00 00 00       	mov    $0xf,%edi
  4013f1:	e8 ba fc ff ff       	call   4010b0 <signal@plt>
  4013f6:	be 70 15 40 00       	mov    $0x401570,%esi
  4013fb:	bf 01 00 00 00       	mov    $0x1,%edi
  401400:	e8 ab fc ff ff       	call   4010b0 <signal@plt>
  401405:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  401409:	c5 fc 11 84 24 e0 00 	vmovups %ymm0,0xe0(%rsp)
  401410:	00 00 
  401412:	c5 fc 11 84 24 d0 00 	vmovups %ymm0,0xd0(%rsp)
  401419:	00 00 
  40141b:	c5 fc 11 84 24 b0 00 	vmovups %ymm0,0xb0(%rsp)
  401422:	00 00 
  401424:	c5 fc 11 84 24 90 00 	vmovups %ymm0,0x90(%rsp)
  40142b:	00 00 
  40142d:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  401433:	48 c7 44 24 68 80 15 	movq   $0x401580,0x68(%rsp)
  40143a:	40 00 
  40143c:	c7 84 24 f0 00 00 00 	movl   $0x4,0xf0(%rsp)
  401443:	04 00 00 00 
  401447:	c5 f8 77             	vzeroupper
  40144a:	e8 d1 fc ff ff       	call   401120 <__libc_current_sigrtmin@plt>
  40144f:	8d 78 01             	lea    0x1(%rax),%edi
  401452:	48 8d 5c 24 68       	lea    0x68(%rsp),%rbx
  401457:	48 89 de             	mov    %rbx,%rsi
  40145a:	31 d2                	xor    %edx,%edx
  40145c:	e8 ef fb ff ff       	call   401050 <sigaction@plt>
  401461:	e8 ba fc ff ff       	call   401120 <__libc_current_sigrtmin@plt>
  401466:	8d 78 02             	lea    0x2(%rax),%edi
  401469:	48 89 de             	mov    %rbx,%rsi
  40146c:	31 d2                	xor    %edx,%edx
  40146e:	e8 dd fb ff ff       	call   401050 <sigaction@plt>
  401473:	e8 a8 fc ff ff       	call   401120 <__libc_current_sigrtmin@plt>
  401478:	ff c0                	inc    %eax
  40147a:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  40147e:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
  401483:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
  401489:	89 44 24 08          	mov    %eax,0x8(%rsp)
  40148d:	48 89 e6             	mov    %rsp,%rsi
  401490:	ba d8 40 40 00       	mov    $0x4040d8,%edx
  401495:	31 ff                	xor    %edi,%edi
  401497:	c5 f8 77             	vzeroupper
  40149a:	e8 d1 fb ff ff       	call   401070 <timer_create@plt>
  40149f:	85 c0                	test   %eax,%eax
  4014a1:	75 27                	jne    4014ca <__rt_init+0xfa>
  4014a3:	c4 e2 7d 1a 05 74 0b 	vbroadcastf128 0xb74(%rip),%ymm0        # 402020 <_IO_stdin_used+0x20>
  4014aa:	00 00 
  4014ac:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
  4014b2:	48 8b 3d 1f 2c 00 00 	mov    0x2c1f(%rip),%rdi        # 4040d8 <g_timer_1hz>
  4014b9:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
  4014be:	31 f6                	xor    %esi,%esi
  4014c0:	31 c9                	xor    %ecx,%ecx
  4014c2:	c5 f8 77             	vzeroupper
  4014c5:	e8 76 fb ff ff       	call   401040 <timer_settime@plt>
  4014ca:	e8 51 fc ff ff       	call   401120 <__libc_current_sigrtmin@plt>
  4014cf:	83 c0 02             	add    $0x2,%eax
  4014d2:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4014d6:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
  4014db:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
  4014e1:	89 44 24 08          	mov    %eax,0x8(%rsp)
  4014e5:	48 89 e6             	mov    %rsp,%rsi
  4014e8:	ba e0 40 40 00       	mov    $0x4040e0,%edx
  4014ed:	31 ff                	xor    %edi,%edi
  4014ef:	c5 f8 77             	vzeroupper
  4014f2:	e8 79 fb ff ff       	call   401070 <timer_create@plt>
  4014f7:	85 c0                	test   %eax,%eax
  4014f9:	75 27                	jne    401522 <__rt_init+0x152>
  4014fb:	c4 e2 7d 1a 05 2c 0b 	vbroadcastf128 0xb2c(%rip),%ymm0        # 402030 <_IO_stdin_used+0x30>
  401502:	00 00 
  401504:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
  40150a:	48 8b 3d cf 2b 00 00 	mov    0x2bcf(%rip),%rdi        # 4040e0 <g_timer_100hz>
  401511:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
  401516:	31 f6                	xor    %esi,%esi
  401518:	31 c9                	xor    %ecx,%ecx
  40151a:	c5 f8 77             	vzeroupper
  40151d:	e8 1e fb ff ff       	call   401040 <timer_settime@plt>
  401522:	48 8b 05 9f 2a 00 00 	mov    0x2a9f(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401529:	48 8b 38             	mov    (%rax),%rdi
  40152c:	31 f6                	xor    %esi,%esi
  40152e:	ba 01 00 00 00       	mov    $0x1,%edx
  401533:	31 c9                	xor    %ecx,%ecx
  401535:	e8 d6 fb ff ff       	call   401110 <setvbuf@plt>
  40153a:	c6 05 70 2b 00 00 01 	movb   $0x1,0x2b70(%rip)        # 4040b1 <__io_pending>
  401541:	48 81 c4 00 01 00 00 	add    $0x100,%rsp
  401548:	5b                   	pop    %rbx
  401549:	c3                   	ret
  40154a:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

0000000000401550 <handle_sigint>:
  401550:	c6 05 5b 2b 00 00 01 	movb   $0x1,0x2b5b(%rip)        # 4040b2 <__sigint_flag>
  401557:	c6 05 53 2b 00 00 01 	movb   $0x1,0x2b53(%rip)        # 4040b1 <__io_pending>
  40155e:	c3                   	ret
  40155f:	90                   	nop

0000000000401560 <handle_sigterm>:
  401560:	c6 05 4c 2b 00 00 01 	movb   $0x1,0x2b4c(%rip)        # 4040b3 <__sigterm_flag>
  401567:	c6 05 43 2b 00 00 01 	movb   $0x1,0x2b43(%rip)        # 4040b1 <__io_pending>
  40156e:	c3                   	ret
  40156f:	90                   	nop

0000000000401570 <handle_sighup>:
  401570:	c6 05 3d 2b 00 00 01 	movb   $0x1,0x2b3d(%rip)        # 4040b4 <__sighup_flag>
  401577:	c6 05 33 2b 00 00 01 	movb   $0x1,0x2b33(%rip)        # 4040b1 <__io_pending>
  40157e:	c3                   	ret
  40157f:	90                   	nop

0000000000401580 <handle_timer>:
  401580:	48 ff 05 31 2b 00 00 	incq   0x2b31(%rip)        # 4040b8 <__timer_1hz>
  401587:	c6 05 23 2b 00 00 01 	movb   $0x1,0x2b23(%rip)        # 4040b1 <__io_pending>
  40158e:	c3                   	ret
  40158f:	90                   	nop

0000000000401590 <__get_env_int>:
  401590:	53                   	push   %rbx
  401591:	48 83 ec 10          	sub    $0x10,%rsp
  401595:	e8 96 fa ff ff       	call   401030 <getenv@plt>
  40159a:	48 85 c0             	test   %rax,%rax
  40159d:	74 32                	je     4015d1 <__get_env_int+0x41>
  40159f:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
  4015a6:	00 00 
  4015a8:	48 8d 74 24 08       	lea    0x8(%rsp),%rsi
  4015ad:	48 89 c7             	mov    %rax,%rdi
  4015b0:	ba 0a 00 00 00       	mov    $0xa,%edx
  4015b5:	48 89 c3             	mov    %rax,%rbx
  4015b8:	e8 13 fb ff ff       	call   4010d0 <strtol@plt>
  4015bd:	48 89 c1             	mov    %rax,%rcx
  4015c0:	31 c0                	xor    %eax,%eax
  4015c2:	48 39 5c 24 08       	cmp    %rbx,0x8(%rsp)
  4015c7:	48 0f 45 c1          	cmovne %rcx,%rax
  4015cb:	48 83 c4 10          	add    $0x10,%rsp
  4015cf:	5b                   	pop    %rbx
  4015d0:	c3                   	ret
  4015d1:	31 c0                	xor    %eax,%eax
  4015d3:	48 83 c4 10          	add    $0x10,%rsp
  4015d7:	5b                   	pop    %rbx
  4015d8:	c3                   	ret
  4015d9:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

00000000004015e0 <__rt_wait>:
  4015e0:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
  4015e7:	8b 3d bb 2a 00 00    	mov    0x2abb(%rip),%edi        # 4040a8 <g_epoll_fd>
  4015ed:	85 ff                	test   %edi,%edi
  4015ef:	79 3f                	jns    401630 <__rt_wait+0x50>
  4015f1:	31 ff                	xor    %edi,%edi
  4015f3:	e8 58 fb ff ff       	call   401150 <epoll_create1@plt>
  4015f8:	89 05 aa 2a 00 00    	mov    %eax,0x2aaa(%rip)        # 4040a8 <g_epoll_fd>
  4015fe:	85 c0                	test   %eax,%eax
  401600:	0f 88 d5 00 00 00    	js     4016db <__rt_wait+0xfb>
  401606:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
  40160d:	00 
  40160e:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
  401615:	00 00 
  401617:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
  40161c:	89 c7                	mov    %eax,%edi
  40161e:	be 01 00 00 00       	mov    $0x1,%esi
  401623:	31 d2                	xor    %edx,%edx
  401625:	e8 66 fa ff ff       	call   401090 <epoll_ctl@plt>
  40162a:	8b 3d 78 2a 00 00    	mov    0x2a78(%rip),%edi        # 4040a8 <g_epoll_fd>
  401630:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  401635:	ba 40 00 00 00       	mov    $0x40,%edx
  40163a:	b9 64 00 00 00       	mov    $0x64,%ecx
  40163f:	e8 bc fa ff ff       	call   401100 <epoll_wait@plt>
  401644:	85 c0                	test   %eax,%eax
  401646:	0f 8e ee 00 00 00    	jle    40173a <__rt_wait+0x15a>
  40164c:	89 c1                	mov    %eax,%ecx
  40164e:	83 f8 01             	cmp    $0x1,%eax
  401651:	75 1e                	jne    401671 <__rt_wait+0x91>
  401653:	31 c0                	xor    %eax,%eax
  401655:	f6 c1 01             	test   $0x1,%cl
  401658:	74 0f                	je     401669 <__rt_wait+0x89>
  40165a:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
  40165e:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
  401663:	0f 84 e0 00 00 00    	je     401749 <__rt_wait+0x169>
  401669:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  401670:	c3                   	ret
  401671:	89 c8                	mov    %ecx,%eax
  401673:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
  401678:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
  40167d:	48 89 c6             	mov    %rax,%rsi
  401680:	eb 18                	jmp    40169a <__rt_wait+0xba>
  401682:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401689:	1f 84 00 00 00 00 00 
  401690:	48 83 c2 18          	add    $0x18,%rdx
  401694:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
  401698:	74 bb                	je     401655 <__rt_wait+0x75>
  40169a:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
  40169e:	75 20                	jne    4016c0 <__rt_wait+0xe0>
  4016a0:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
  4016a4:	74 1a                	je     4016c0 <__rt_wait+0xe0>
  4016a6:	c6 05 03 2a 00 00 01 	movb   $0x1,0x2a03(%rip)        # 4040b0 <__stdin_ready>
  4016ad:	c6 05 fd 29 00 00 01 	movb   $0x1,0x29fd(%rip)        # 4040b1 <__io_pending>
  4016b4:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
  4016bb:	00 00 00 00 00 
  4016c0:	83 3a 00             	cmpl   $0x0,(%rdx)
  4016c3:	75 cb                	jne    401690 <__rt_wait+0xb0>
  4016c5:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
  4016c9:	74 c5                	je     401690 <__rt_wait+0xb0>
  4016cb:	c6 05 de 29 00 00 01 	movb   $0x1,0x29de(%rip)        # 4040b0 <__stdin_ready>
  4016d2:	c6 05 d8 29 00 00 01 	movb   $0x1,0x29d8(%rip)        # 4040b1 <__io_pending>
  4016d9:	eb b5                	jmp    401690 <__rt_wait+0xb0>
  4016db:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4016df:	c5 fc 11 44 24 30    	vmovups %ymm0,0x30(%rsp)
  4016e5:	c5 fc 11 44 24 50    	vmovups %ymm0,0x50(%rsp)
  4016eb:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  4016f1:	c5 f8 28 05 27 09 00 	vmovaps 0x927(%rip),%xmm0        # 402020 <_IO_stdin_used+0x20>
  4016f8:	00 
  4016f9:	c5 fc 11 44 24 10    	vmovups %ymm0,0x10(%rsp)
  4016ff:	c5 f8 10 05 39 09 00 	vmovups 0x939(%rip),%xmm0        # 402040 <_IO_stdin_used+0x40>
  401706:	00 
  401707:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
  40170c:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  401711:	49 89 e0             	mov    %rsp,%r8
  401714:	bf 01 00 00 00       	mov    $0x1,%edi
  401719:	31 d2                	xor    %edx,%edx
  40171b:	31 c9                	xor    %ecx,%ecx
  40171d:	c5 f8 77             	vzeroupper
  401720:	e8 bb f9 ff ff       	call   4010e0 <select@plt>
  401725:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
  40172a:	74 0e                	je     40173a <__rt_wait+0x15a>
  40172c:	c6 05 7d 29 00 00 01 	movb   $0x1,0x297d(%rip)        # 4040b0 <__stdin_ready>
  401733:	c6 05 77 29 00 00 01 	movb   $0x1,0x2977(%rip)        # 4040b1 <__io_pending>
  40173a:	c6 05 70 29 00 00 01 	movb   $0x1,0x2970(%rip)        # 4040b1 <__io_pending>
  401741:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  401748:	c3                   	ret
  401749:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
  40174e:	0f 84 15 ff ff ff    	je     401669 <__rt_wait+0x89>
  401754:	c6 05 55 29 00 00 01 	movb   $0x1,0x2955(%rip)        # 4040b0 <__stdin_ready>
  40175b:	c6 05 4f 29 00 00 01 	movb   $0x1,0x294f(%rip)        # 4040b1 <__io_pending>
  401762:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  401769:	c3                   	ret
  40176a:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

0000000000401770 <__rt_poll>:
  401770:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
  401777:	8b 3d 2b 29 00 00    	mov    0x292b(%rip),%edi        # 4040a8 <g_epoll_fd>
  40177d:	85 ff                	test   %edi,%edi
  40177f:	79 3f                	jns    4017c0 <__rt_poll+0x50>
  401781:	31 ff                	xor    %edi,%edi
  401783:	e8 c8 f9 ff ff       	call   401150 <epoll_create1@plt>
  401788:	89 05 1a 29 00 00    	mov    %eax,0x291a(%rip)        # 4040a8 <g_epoll_fd>
  40178e:	85 c0                	test   %eax,%eax
  401790:	0f 88 d5 00 00 00    	js     40186b <__rt_poll+0xfb>
  401796:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
  40179d:	00 
  40179e:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
  4017a5:	00 00 
  4017a7:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
  4017ac:	89 c7                	mov    %eax,%edi
  4017ae:	be 01 00 00 00       	mov    $0x1,%esi
  4017b3:	31 d2                	xor    %edx,%edx
  4017b5:	e8 d6 f8 ff ff       	call   401090 <epoll_ctl@plt>
  4017ba:	8b 3d e8 28 00 00    	mov    0x28e8(%rip),%edi        # 4040a8 <g_epoll_fd>
  4017c0:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  4017c5:	ba 40 00 00 00       	mov    $0x40,%edx
  4017ca:	31 c9                	xor    %ecx,%ecx
  4017cc:	e8 2f f9 ff ff       	call   401100 <epoll_wait@plt>
  4017d1:	85 c0                	test   %eax,%eax
  4017d3:	7e 1d                	jle    4017f2 <__rt_poll+0x82>
  4017d5:	89 c1                	mov    %eax,%ecx
  4017d7:	83 f8 01             	cmp    $0x1,%eax
  4017da:	75 25                	jne    401801 <__rt_poll+0x91>
  4017dc:	31 c0                	xor    %eax,%eax
  4017de:	f6 c1 01             	test   $0x1,%cl
  4017e1:	74 0f                	je     4017f2 <__rt_poll+0x82>
  4017e3:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
  4017e7:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
  4017ec:	0f 84 cc 00 00 00    	je     4018be <__rt_poll+0x14e>
  4017f2:	c6 05 b8 28 00 00 01 	movb   $0x1,0x28b8(%rip)        # 4040b1 <__io_pending>
  4017f9:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  401800:	c3                   	ret
  401801:	89 c8                	mov    %ecx,%eax
  401803:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
  401808:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
  40180d:	48 89 c6             	mov    %rax,%rsi
  401810:	eb 18                	jmp    40182a <__rt_poll+0xba>
  401812:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401819:	1f 84 00 00 00 00 00 
  401820:	48 83 c2 18          	add    $0x18,%rdx
  401824:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
  401828:	74 b4                	je     4017de <__rt_poll+0x6e>
  40182a:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
  40182e:	75 20                	jne    401850 <__rt_poll+0xe0>
  401830:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
  401834:	74 1a                	je     401850 <__rt_poll+0xe0>
  401836:	c6 05 73 28 00 00 01 	movb   $0x1,0x2873(%rip)        # 4040b0 <__stdin_ready>
  40183d:	c6 05 6d 28 00 00 01 	movb   $0x1,0x286d(%rip)        # 4040b1 <__io_pending>
  401844:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
  40184b:	00 00 00 00 00 
  401850:	83 3a 00             	cmpl   $0x0,(%rdx)
  401853:	75 cb                	jne    401820 <__rt_poll+0xb0>
  401855:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
  401859:	74 c5                	je     401820 <__rt_poll+0xb0>
  40185b:	c6 05 4e 28 00 00 01 	movb   $0x1,0x284e(%rip)        # 4040b0 <__stdin_ready>
  401862:	c6 05 48 28 00 00 01 	movb   $0x1,0x2848(%rip)        # 4040b1 <__io_pending>
  401869:	eb b5                	jmp    401820 <__rt_poll+0xb0>
  40186b:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  40186f:	c5 fc 11 44 24 30    	vmovups %ymm0,0x30(%rsp)
  401875:	c5 fc 11 44 24 50    	vmovups %ymm0,0x50(%rsp)
  40187b:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  401881:	c5 f8 28 05 97 07 00 	vmovaps 0x797(%rip),%xmm0        # 402020 <_IO_stdin_used+0x20>
  401888:	00 
  401889:	c5 fc 11 44 24 10    	vmovups %ymm0,0x10(%rsp)
  40188f:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  401893:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
  401898:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  40189d:	49 89 e0             	mov    %rsp,%r8
  4018a0:	bf 01 00 00 00       	mov    $0x1,%edi
  4018a5:	31 d2                	xor    %edx,%edx
  4018a7:	31 c9                	xor    %ecx,%ecx
  4018a9:	c5 f8 77             	vzeroupper
  4018ac:	e8 2f f8 ff ff       	call   4010e0 <select@plt>
  4018b1:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
  4018b6:	0f 84 36 ff ff ff    	je     4017f2 <__rt_poll+0x82>
  4018bc:	eb 0b                	jmp    4018c9 <__rt_poll+0x159>
  4018be:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
  4018c3:	0f 84 29 ff ff ff    	je     4017f2 <__rt_poll+0x82>
  4018c9:	c6 05 e0 27 00 00 01 	movb   $0x1,0x27e0(%rip)        # 4040b0 <__stdin_ready>
  4018d0:	c6 05 da 27 00 00 01 	movb   $0x1,0x27da(%rip)        # 4040b1 <__io_pending>
  4018d7:	c6 05 d3 27 00 00 01 	movb   $0x1,0x27d3(%rip)        # 4040b1 <__io_pending>
  4018de:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  4018e5:	c3                   	ret
  4018e6:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  4018ed:	00 00 00 

00000000004018f0 <__wait_for_event>:
  4018f0:	e9 eb fc ff ff       	jmp    4015e0 <__rt_wait>
  4018f5:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  4018fc:	00 00 00 00 

0000000000401900 <__print>:
  401900:	50                   	push   %rax
  401901:	48 8b 05 c0 26 00 00 	mov    0x26c0(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401908:	48 8b 30             	mov    (%rax),%rsi
  40190b:	e8 70 f7 ff ff       	call   401080 <fputs@plt>
  401910:	b8 01 00 00 00       	mov    $0x1,%eax
  401915:	59                   	pop    %rcx
  401916:	c3                   	ret
  401917:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  40191e:	00 00 

0000000000401920 <__print_int>:
  401920:	50                   	push   %rax
  401921:	48 89 fa             	mov    %rdi,%rdx
  401924:	48 8b 05 b5 26 00 00 	mov    0x26b5(%rip),%rax        # 403fe0 <stderr@GLIBC_2.2.5>
  40192b:	48 8b 38             	mov    (%rax),%rdi
  40192e:	be be 20 40 00       	mov    $0x4020be,%esi
  401933:	31 c0                	xor    %eax,%eax
  401935:	e8 86 f7 ff ff       	call   4010c0 <fprintf@plt>
  40193a:	b8 01 00 00 00       	mov    $0x1,%eax
  40193f:	59                   	pop    %rcx
  401940:	c3                   	ret
  401941:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401948:	0f 1f 84 00 00 00 00 
  40194f:	00 

0000000000401950 <__print_float>:
  401950:	50                   	push   %rax
  401951:	48 8b 05 88 26 00 00 	mov    0x2688(%rip),%rax        # 403fe0 <stderr@GLIBC_2.2.5>
  401958:	48 8b 38             	mov    (%rax),%rdi
  40195b:	c5 fa 5a c0          	vcvtss2sd %xmm0,%xmm0,%xmm0
  40195f:	be c4 20 40 00       	mov    $0x4020c4,%esi
  401964:	b0 01                	mov    $0x1,%al
  401966:	e8 55 f7 ff ff       	call   4010c0 <fprintf@plt>
  40196b:	b8 01 00 00 00       	mov    $0x1,%eax
  401970:	59                   	pop    %rcx
  401971:	c3                   	ret
  401972:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401979:	1f 84 00 00 00 00 00 

0000000000401980 <__sqrtf>:
  401980:	e9 6b f7 ff ff       	jmp    4010f0 <sqrtf@plt>
  401985:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  40198c:	00 00 00 00 

0000000000401990 <__exit>:
  401990:	50                   	push   %rax
  401991:	31 ff                	xor    %edi,%edi
  401993:	e8 98 f7 ff ff       	call   401130 <exit@plt>
  401998:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  40199f:	00 

00000000004019a0 <__print_str_len>:
  4019a0:	53                   	push   %rbx
  4019a1:	48 89 f3             	mov    %rsi,%rbx
  4019a4:	48 8b 05 1d 26 00 00 	mov    0x261d(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  4019ab:	48 8b 08             	mov    (%rax),%rcx
  4019ae:	be 01 00 00 00       	mov    $0x1,%esi
  4019b3:	48 89 da             	mov    %rbx,%rdx
  4019b6:	e8 85 f7 ff ff       	call   401140 <fwrite@plt>
  4019bb:	48 89 d8             	mov    %rbx,%rax
  4019be:	5b                   	pop    %rbx
  4019bf:	c3                   	ret

00000000004019c0 <__write_bytes>:
  4019c0:	53                   	push   %rbx
  4019c1:	48 89 f3             	mov    %rsi,%rbx
  4019c4:	48 8b 05 fd 25 00 00 	mov    0x25fd(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  4019cb:	48 8b 08             	mov    (%rax),%rcx
  4019ce:	be 01 00 00 00       	mov    $0x1,%esi
  4019d3:	48 89 da             	mov    %rbx,%rdx
  4019d6:	e8 65 f7 ff ff       	call   401140 <fwrite@plt>
  4019db:	48 89 d8             	mov    %rbx,%rax
  4019de:	5b                   	pop    %rbx
  4019df:	c3                   	ret

00000000004019e0 <__read_stdin>:
  4019e0:	48 89 f2             	mov    %rsi,%rdx
  4019e3:	48 8b 05 e6 25 00 00 	mov    0x25e6(%rip),%rax        # 403fd0 <stdin@GLIBC_2.2.5>
  4019ea:	48 8b 08             	mov    (%rax),%rcx
  4019ed:	be 01 00 00 00       	mov    $0x1,%esi
  4019f2:	e9 69 f6 ff ff       	jmp    401060 <fread@plt>
  4019f7:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  4019fe:	00 00 

0000000000401a00 <__putchar>:
  401a00:	53                   	push   %rbx
  401a01:	48 89 fb             	mov    %rdi,%rbx
  401a04:	48 8b 05 bd 25 00 00 	mov    0x25bd(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401a0b:	48 8b 30             	mov    (%rax),%rsi
  401a0e:	e8 8d f6 ff ff       	call   4010a0 <putc@plt>
  401a13:	48 89 d8             	mov    %rbx,%rax
  401a16:	5b                   	pop    %rbx
  401a17:	c3                   	ret
  401a18:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  401a1f:	00 

0000000000401a20 <brief_thread_pool_init>:
  401a20:	c3                   	ret
  401a21:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401a28:	0f 1f 84 00 00 00 00 
  401a2f:	00 

0000000000401a30 <brief_barrier_release>:
  401a30:	c3                   	ret
  401a31:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401a38:	0f 1f 84 00 00 00 00 
  401a3f:	00 

0000000000401a40 <brief_barrier_wait>:
  401a40:	c3                   	ret
  401a41:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401a48:	0f 1f 84 00 00 00 00 
  401a4f:	00 

0000000000401a50 <brief_thread_pool_shutdown>:
  401a50:	c3                   	ret

Disassembly of section .fini:

0000000000401a54 <_fini>:
  401a54:	f3 0f 1e fa          	endbr64
  401a58:	48 83 ec 08          	sub    $0x8,%rsp
  401a5c:	48 83 c4 08          	add    $0x8,%rsp
  401a60:	c3                   	ret
