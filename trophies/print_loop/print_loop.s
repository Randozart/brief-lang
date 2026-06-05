
benchmarks/print_loop:     file format elf64-x86-64


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
  401178:	48 c7 c7 20 13 40 00 	mov    $0x401320,%rdi
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

0000000000401250 <work>:
  401250:	48 89 f8             	mov    %rdi,%rax
  401253:	0f b6 0d 57 2e 00 00 	movzbl 0x2e57(%rip),%ecx        # 4040b1 <__io_pending>
  40125a:	48 8b 3f             	mov    (%rdi),%rdi
  40125d:	48 ff c7             	inc    %rdi
  401260:	48 89 38             	mov    %rdi,(%rax)
  401263:	48 b8 1d e6 cb 0b b0 	movabs $0x5d4e8fb00bcbe61d,%rax
  40126a:	8f 4e 5d 
  40126d:	48 0f af c7          	imul   %rdi,%rax
  401271:	48 b9 60 b4 71 c4 5a 	movabs $0xa7c5ac471b460,%rcx
  401278:	7c 0a 00 
  40127b:	48 01 c1             	add    %rax,%rcx
  40127e:	48 0f ac c9 05       	shrd   $0x5,%rcx,%rcx
  401283:	48 b8 46 1b 47 ac c5 	movabs $0xa7c5ac471b46,%rax
  40128a:	a7 00 00 
  40128d:	48 39 c1             	cmp    %rax,%rcx
  401290:	0f 86 0a 07 00 00    	jbe    4019a0 <__print_int>
  401296:	c3                   	ret
  401297:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  40129e:	00 00 

00000000004012a0 <init_state>:
  4012a0:	48 c7 07 00 00 00 00 	movq   $0x0,(%rdi)
  4012a7:	c3                   	ret
  4012a8:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  4012af:	00 

00000000004012b0 <reactor_tick>:
  4012b0:	48 89 f8             	mov    %rdi,%rax
  4012b3:	0f b6 0d f7 2d 00 00 	movzbl 0x2df7(%rip),%ecx        # 4040b1 <__io_pending>
  4012ba:	0f b6 0d f0 2d 00 00 	movzbl 0x2df0(%rip),%ecx        # 4040b1 <__io_pending>
  4012c1:	48 8b 3f             	mov    (%rdi),%rdi
  4012c4:	48 81 ff 7f f0 fa 02 	cmp    $0x2faf07f,%rdi
  4012cb:	7f 45                	jg     401312 <reactor_tick+0x62>
  4012cd:	80 e1 01             	and    $0x1,%cl
  4012d0:	74 40                	je     401312 <reactor_tick+0x62>
  4012d2:	0f b6 0d d8 2d 00 00 	movzbl 0x2dd8(%rip),%ecx        # 4040b1 <__io_pending>
  4012d9:	48 ff c7             	inc    %rdi
  4012dc:	48 89 38             	mov    %rdi,(%rax)
  4012df:	48 b8 1d e6 cb 0b b0 	movabs $0x5d4e8fb00bcbe61d,%rax
  4012e6:	8f 4e 5d 
  4012e9:	48 0f af c7          	imul   %rdi,%rax
  4012ed:	48 b9 60 b4 71 c4 5a 	movabs $0xa7c5ac471b460,%rcx
  4012f4:	7c 0a 00 
  4012f7:	48 01 c1             	add    %rax,%rcx
  4012fa:	48 0f ac c9 05       	shrd   $0x5,%rcx,%rcx
  4012ff:	48 b8 46 1b 47 ac c5 	movabs $0xa7c5ac471b46,%rax
  401306:	a7 00 00 
  401309:	48 39 c1             	cmp    %rax,%rcx
  40130c:	0f 86 8e 06 00 00    	jbe    4019a0 <__print_int>
  401312:	c3                   	ret
  401313:	66 66 66 66 2e 0f 1f 	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  40131a:	84 00 00 00 00 00 

0000000000401320 <main>:
  401320:	41 57                	push   %r15
  401322:	41 56                	push   %r14
  401324:	41 54                	push   %r12
  401326:	53                   	push   %rbx
  401327:	50                   	push   %rax
  401328:	e8 23 01 00 00       	call   401450 <__rt_init>
  40132d:	e8 be 04 00 00       	call   4017f0 <__rt_poll>
  401332:	49 be 1d e6 cb 0b b0 	movabs $0x5d4e8fb00bcbe61d,%r14
  401339:	8f 4e 5d 
  40133c:	49 bf 60 b4 71 c4 5a 	movabs $0xa7c5ac471b460,%r15
  401343:	7c 0a 00 
  401346:	49 bc 46 1b 47 ac c5 	movabs $0xa7c5ac471b46,%r12
  40134d:	a7 00 00 
  401350:	0f b6 05 5a 2d 00 00 	movzbl 0x2d5a(%rip),%eax        # 4040b1 <__io_pending>
  401357:	31 db                	xor    %ebx,%ebx
  401359:	eb 0d                	jmp    401368 <main+0x48>
  40135b:	0f 1f 44 00 00       	nopl   0x0(%rax,%rax,1)
  401360:	48 89 df             	mov    %rbx,%rdi
  401363:	e8 38 06 00 00       	call   4019a0 <__print_int>
  401368:	48 81 fb 7d f0 fa 02 	cmp    $0x2faf07d,%rbx
  40136f:	7d 7f                	jge    4013f0 <main+0xd0>
  401371:	48 8d 7b 01          	lea    0x1(%rbx),%rdi
  401375:	48 89 f8             	mov    %rdi,%rax
  401378:	49 0f af c6          	imul   %r14,%rax
  40137c:	4c 01 f8             	add    %r15,%rax
  40137f:	48 0f ac c0 05       	shrd   $0x5,%rax,%rax
  401384:	4c 39 e0             	cmp    %r12,%rax
  401387:	77 05                	ja     40138e <main+0x6e>
  401389:	e8 12 06 00 00       	call   4019a0 <__print_int>
  40138e:	48 8d 7b 02          	lea    0x2(%rbx),%rdi
  401392:	48 89 f8             	mov    %rdi,%rax
  401395:	49 0f af c6          	imul   %r14,%rax
  401399:	4c 01 f8             	add    %r15,%rax
  40139c:	48 0f ac c0 05       	shrd   $0x5,%rax,%rax
  4013a1:	4c 39 e0             	cmp    %r12,%rax
  4013a4:	77 05                	ja     4013ab <main+0x8b>
  4013a6:	e8 f5 05 00 00       	call   4019a0 <__print_int>
  4013ab:	48 8d 7b 03          	lea    0x3(%rbx),%rdi
  4013af:	48 89 f8             	mov    %rdi,%rax
  4013b2:	49 0f af c6          	imul   %r14,%rax
  4013b6:	4c 01 f8             	add    %r15,%rax
  4013b9:	48 0f ac c0 05       	shrd   $0x5,%rax,%rax
  4013be:	4c 39 e0             	cmp    %r12,%rax
  4013c1:	77 05                	ja     4013c8 <main+0xa8>
  4013c3:	e8 d8 05 00 00       	call   4019a0 <__print_int>
  4013c8:	48 83 c3 04          	add    $0x4,%rbx
  4013cc:	48 89 d8             	mov    %rbx,%rax
  4013cf:	49 0f af c6          	imul   %r14,%rax
  4013d3:	4c 01 f8             	add    %r15,%rax
  4013d6:	48 0f ac c0 05       	shrd   $0x5,%rax,%rax
  4013db:	4c 39 e0             	cmp    %r12,%rax
  4013de:	77 88                	ja     401368 <main+0x48>
  4013e0:	e9 7b ff ff ff       	jmp    401360 <main+0x40>
  4013e5:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  4013ec:	00 00 00 00 
  4013f0:	48 81 fb 80 f0 fa 02 	cmp    $0x2faf080,%rbx
  4013f7:	73 27                	jae    401420 <main+0x100>
  4013f9:	48 ff c3             	inc    %rbx
  4013fc:	69 c3 1d e6 cb 0b    	imul   $0xbcbe61d,%ebx,%eax
  401402:	0f ac c0 05          	shrd   $0x5,%eax,%eax
  401406:	3d c6 a7 00 00       	cmp    $0xa7c6,%eax
  40140b:	0f 83 57 ff ff ff    	jae    401368 <main+0x48>
  401411:	e9 4a ff ff ff       	jmp    401360 <main+0x40>
  401416:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  40141d:	00 00 00 
  401420:	74 0a                	je     40142c <main+0x10c>
  401422:	e8 39 02 00 00       	call   401660 <__rt_wait>
  401427:	e9 24 ff ff ff       	jmp    401350 <main+0x30>
  40142c:	31 c0                	xor    %eax,%eax
  40142e:	48 83 c4 08          	add    $0x8,%rsp
  401432:	5b                   	pop    %rbx
  401433:	41 5c                	pop    %r12
  401435:	41 5e                	pop    %r14
  401437:	41 5f                	pop    %r15
  401439:	c3                   	ret
  40143a:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

0000000000401440 <brief_rt_ctor>:
  401440:	e9 0b 00 00 00       	jmp    401450 <__rt_init>
  401445:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  40144c:	00 00 00 00 

0000000000401450 <__rt_init>:
  401450:	53                   	push   %rbx
  401451:	48 81 ec 00 01 00 00 	sub    $0x100,%rsp
  401458:	be d0 15 40 00       	mov    $0x4015d0,%esi
  40145d:	bf 02 00 00 00       	mov    $0x2,%edi
  401462:	e8 49 fc ff ff       	call   4010b0 <signal@plt>
  401467:	be e0 15 40 00       	mov    $0x4015e0,%esi
  40146c:	bf 0f 00 00 00       	mov    $0xf,%edi
  401471:	e8 3a fc ff ff       	call   4010b0 <signal@plt>
  401476:	be f0 15 40 00       	mov    $0x4015f0,%esi
  40147b:	bf 01 00 00 00       	mov    $0x1,%edi
  401480:	e8 2b fc ff ff       	call   4010b0 <signal@plt>
  401485:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  401489:	c5 fc 11 84 24 e0 00 	vmovups %ymm0,0xe0(%rsp)
  401490:	00 00 
  401492:	c5 fc 11 84 24 d0 00 	vmovups %ymm0,0xd0(%rsp)
  401499:	00 00 
  40149b:	c5 fc 11 84 24 b0 00 	vmovups %ymm0,0xb0(%rsp)
  4014a2:	00 00 
  4014a4:	c5 fc 11 84 24 90 00 	vmovups %ymm0,0x90(%rsp)
  4014ab:	00 00 
  4014ad:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  4014b3:	48 c7 44 24 68 00 16 	movq   $0x401600,0x68(%rsp)
  4014ba:	40 00 
  4014bc:	c7 84 24 f0 00 00 00 	movl   $0x4,0xf0(%rsp)
  4014c3:	04 00 00 00 
  4014c7:	c5 f8 77             	vzeroupper
  4014ca:	e8 51 fc ff ff       	call   401120 <__libc_current_sigrtmin@plt>
  4014cf:	8d 78 01             	lea    0x1(%rax),%edi
  4014d2:	48 8d 5c 24 68       	lea    0x68(%rsp),%rbx
  4014d7:	48 89 de             	mov    %rbx,%rsi
  4014da:	31 d2                	xor    %edx,%edx
  4014dc:	e8 6f fb ff ff       	call   401050 <sigaction@plt>
  4014e1:	e8 3a fc ff ff       	call   401120 <__libc_current_sigrtmin@plt>
  4014e6:	8d 78 02             	lea    0x2(%rax),%edi
  4014e9:	48 89 de             	mov    %rbx,%rsi
  4014ec:	31 d2                	xor    %edx,%edx
  4014ee:	e8 5d fb ff ff       	call   401050 <sigaction@plt>
  4014f3:	e8 28 fc ff ff       	call   401120 <__libc_current_sigrtmin@plt>
  4014f8:	ff c0                	inc    %eax
  4014fa:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4014fe:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
  401503:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
  401509:	89 44 24 08          	mov    %eax,0x8(%rsp)
  40150d:	48 89 e6             	mov    %rsp,%rsi
  401510:	ba d8 40 40 00       	mov    $0x4040d8,%edx
  401515:	31 ff                	xor    %edi,%edi
  401517:	c5 f8 77             	vzeroupper
  40151a:	e8 51 fb ff ff       	call   401070 <timer_create@plt>
  40151f:	85 c0                	test   %eax,%eax
  401521:	75 27                	jne    40154a <__rt_init+0xfa>
  401523:	c4 e2 7d 1a 05 34 0b 	vbroadcastf128 0xb34(%rip),%ymm0        # 402060 <_IO_stdin_used+0x60>
  40152a:	00 00 
  40152c:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
  401532:	48 8b 3d 9f 2b 00 00 	mov    0x2b9f(%rip),%rdi        # 4040d8 <g_timer_1hz>
  401539:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
  40153e:	31 f6                	xor    %esi,%esi
  401540:	31 c9                	xor    %ecx,%ecx
  401542:	c5 f8 77             	vzeroupper
  401545:	e8 f6 fa ff ff       	call   401040 <timer_settime@plt>
  40154a:	e8 d1 fb ff ff       	call   401120 <__libc_current_sigrtmin@plt>
  40154f:	83 c0 02             	add    $0x2,%eax
  401552:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  401556:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
  40155b:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
  401561:	89 44 24 08          	mov    %eax,0x8(%rsp)
  401565:	48 89 e6             	mov    %rsp,%rsi
  401568:	ba e0 40 40 00       	mov    $0x4040e0,%edx
  40156d:	31 ff                	xor    %edi,%edi
  40156f:	c5 f8 77             	vzeroupper
  401572:	e8 f9 fa ff ff       	call   401070 <timer_create@plt>
  401577:	85 c0                	test   %eax,%eax
  401579:	75 27                	jne    4015a2 <__rt_init+0x152>
  40157b:	c4 e2 7d 1a 05 ec 0a 	vbroadcastf128 0xaec(%rip),%ymm0        # 402070 <_IO_stdin_used+0x70>
  401582:	00 00 
  401584:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
  40158a:	48 8b 3d 4f 2b 00 00 	mov    0x2b4f(%rip),%rdi        # 4040e0 <g_timer_100hz>
  401591:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
  401596:	31 f6                	xor    %esi,%esi
  401598:	31 c9                	xor    %ecx,%ecx
  40159a:	c5 f8 77             	vzeroupper
  40159d:	e8 9e fa ff ff       	call   401040 <timer_settime@plt>
  4015a2:	48 8b 05 1f 2a 00 00 	mov    0x2a1f(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  4015a9:	48 8b 38             	mov    (%rax),%rdi
  4015ac:	31 f6                	xor    %esi,%esi
  4015ae:	ba 01 00 00 00       	mov    $0x1,%edx
  4015b3:	31 c9                	xor    %ecx,%ecx
  4015b5:	e8 56 fb ff ff       	call   401110 <setvbuf@plt>
  4015ba:	c6 05 f0 2a 00 00 01 	movb   $0x1,0x2af0(%rip)        # 4040b1 <__io_pending>
  4015c1:	48 81 c4 00 01 00 00 	add    $0x100,%rsp
  4015c8:	5b                   	pop    %rbx
  4015c9:	c3                   	ret
  4015ca:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

00000000004015d0 <handle_sigint>:
  4015d0:	c6 05 db 2a 00 00 01 	movb   $0x1,0x2adb(%rip)        # 4040b2 <__sigint_flag>
  4015d7:	c6 05 d3 2a 00 00 01 	movb   $0x1,0x2ad3(%rip)        # 4040b1 <__io_pending>
  4015de:	c3                   	ret
  4015df:	90                   	nop

00000000004015e0 <handle_sigterm>:
  4015e0:	c6 05 cc 2a 00 00 01 	movb   $0x1,0x2acc(%rip)        # 4040b3 <__sigterm_flag>
  4015e7:	c6 05 c3 2a 00 00 01 	movb   $0x1,0x2ac3(%rip)        # 4040b1 <__io_pending>
  4015ee:	c3                   	ret
  4015ef:	90                   	nop

00000000004015f0 <handle_sighup>:
  4015f0:	c6 05 bd 2a 00 00 01 	movb   $0x1,0x2abd(%rip)        # 4040b4 <__sighup_flag>
  4015f7:	c6 05 b3 2a 00 00 01 	movb   $0x1,0x2ab3(%rip)        # 4040b1 <__io_pending>
  4015fe:	c3                   	ret
  4015ff:	90                   	nop

0000000000401600 <handle_timer>:
  401600:	48 ff 05 b1 2a 00 00 	incq   0x2ab1(%rip)        # 4040b8 <__timer_1hz>
  401607:	c6 05 a3 2a 00 00 01 	movb   $0x1,0x2aa3(%rip)        # 4040b1 <__io_pending>
  40160e:	c3                   	ret
  40160f:	90                   	nop

0000000000401610 <__get_env_int>:
  401610:	53                   	push   %rbx
  401611:	48 83 ec 10          	sub    $0x10,%rsp
  401615:	e8 16 fa ff ff       	call   401030 <getenv@plt>
  40161a:	48 85 c0             	test   %rax,%rax
  40161d:	74 32                	je     401651 <__get_env_int+0x41>
  40161f:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
  401626:	00 00 
  401628:	48 8d 74 24 08       	lea    0x8(%rsp),%rsi
  40162d:	48 89 c7             	mov    %rax,%rdi
  401630:	ba 0a 00 00 00       	mov    $0xa,%edx
  401635:	48 89 c3             	mov    %rax,%rbx
  401638:	e8 93 fa ff ff       	call   4010d0 <strtol@plt>
  40163d:	48 89 c1             	mov    %rax,%rcx
  401640:	31 c0                	xor    %eax,%eax
  401642:	48 39 5c 24 08       	cmp    %rbx,0x8(%rsp)
  401647:	48 0f 45 c1          	cmovne %rcx,%rax
  40164b:	48 83 c4 10          	add    $0x10,%rsp
  40164f:	5b                   	pop    %rbx
  401650:	c3                   	ret
  401651:	31 c0                	xor    %eax,%eax
  401653:	48 83 c4 10          	add    $0x10,%rsp
  401657:	5b                   	pop    %rbx
  401658:	c3                   	ret
  401659:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000401660 <__rt_wait>:
  401660:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
  401667:	8b 3d 3b 2a 00 00    	mov    0x2a3b(%rip),%edi        # 4040a8 <g_epoll_fd>
  40166d:	85 ff                	test   %edi,%edi
  40166f:	79 3f                	jns    4016b0 <__rt_wait+0x50>
  401671:	31 ff                	xor    %edi,%edi
  401673:	e8 d8 fa ff ff       	call   401150 <epoll_create1@plt>
  401678:	89 05 2a 2a 00 00    	mov    %eax,0x2a2a(%rip)        # 4040a8 <g_epoll_fd>
  40167e:	85 c0                	test   %eax,%eax
  401680:	0f 88 d5 00 00 00    	js     40175b <__rt_wait+0xfb>
  401686:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
  40168d:	00 
  40168e:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
  401695:	00 00 
  401697:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
  40169c:	89 c7                	mov    %eax,%edi
  40169e:	be 01 00 00 00       	mov    $0x1,%esi
  4016a3:	31 d2                	xor    %edx,%edx
  4016a5:	e8 e6 f9 ff ff       	call   401090 <epoll_ctl@plt>
  4016aa:	8b 3d f8 29 00 00    	mov    0x29f8(%rip),%edi        # 4040a8 <g_epoll_fd>
  4016b0:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  4016b5:	ba 40 00 00 00       	mov    $0x40,%edx
  4016ba:	b9 64 00 00 00       	mov    $0x64,%ecx
  4016bf:	e8 3c fa ff ff       	call   401100 <epoll_wait@plt>
  4016c4:	85 c0                	test   %eax,%eax
  4016c6:	0f 8e ee 00 00 00    	jle    4017ba <__rt_wait+0x15a>
  4016cc:	89 c1                	mov    %eax,%ecx
  4016ce:	83 f8 01             	cmp    $0x1,%eax
  4016d1:	75 1e                	jne    4016f1 <__rt_wait+0x91>
  4016d3:	31 c0                	xor    %eax,%eax
  4016d5:	f6 c1 01             	test   $0x1,%cl
  4016d8:	74 0f                	je     4016e9 <__rt_wait+0x89>
  4016da:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
  4016de:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
  4016e3:	0f 84 e0 00 00 00    	je     4017c9 <__rt_wait+0x169>
  4016e9:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  4016f0:	c3                   	ret
  4016f1:	89 c8                	mov    %ecx,%eax
  4016f3:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
  4016f8:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
  4016fd:	48 89 c6             	mov    %rax,%rsi
  401700:	eb 18                	jmp    40171a <__rt_wait+0xba>
  401702:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401709:	1f 84 00 00 00 00 00 
  401710:	48 83 c2 18          	add    $0x18,%rdx
  401714:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
  401718:	74 bb                	je     4016d5 <__rt_wait+0x75>
  40171a:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
  40171e:	75 20                	jne    401740 <__rt_wait+0xe0>
  401720:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
  401724:	74 1a                	je     401740 <__rt_wait+0xe0>
  401726:	c6 05 83 29 00 00 01 	movb   $0x1,0x2983(%rip)        # 4040b0 <__stdin_ready>
  40172d:	c6 05 7d 29 00 00 01 	movb   $0x1,0x297d(%rip)        # 4040b1 <__io_pending>
  401734:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
  40173b:	00 00 00 00 00 
  401740:	83 3a 00             	cmpl   $0x0,(%rdx)
  401743:	75 cb                	jne    401710 <__rt_wait+0xb0>
  401745:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
  401749:	74 c5                	je     401710 <__rt_wait+0xb0>
  40174b:	c6 05 5e 29 00 00 01 	movb   $0x1,0x295e(%rip)        # 4040b0 <__stdin_ready>
  401752:	c6 05 58 29 00 00 01 	movb   $0x1,0x2958(%rip)        # 4040b1 <__io_pending>
  401759:	eb b5                	jmp    401710 <__rt_wait+0xb0>
  40175b:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  40175f:	c5 fc 11 44 24 30    	vmovups %ymm0,0x30(%rsp)
  401765:	c5 fc 11 44 24 50    	vmovups %ymm0,0x50(%rsp)
  40176b:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  401771:	c5 f8 28 05 e7 08 00 	vmovaps 0x8e7(%rip),%xmm0        # 402060 <_IO_stdin_used+0x60>
  401778:	00 
  401779:	c5 fc 11 44 24 10    	vmovups %ymm0,0x10(%rsp)
  40177f:	c5 f8 10 05 f9 08 00 	vmovups 0x8f9(%rip),%xmm0        # 402080 <_IO_stdin_used+0x80>
  401786:	00 
  401787:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
  40178c:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  401791:	49 89 e0             	mov    %rsp,%r8
  401794:	bf 01 00 00 00       	mov    $0x1,%edi
  401799:	31 d2                	xor    %edx,%edx
  40179b:	31 c9                	xor    %ecx,%ecx
  40179d:	c5 f8 77             	vzeroupper
  4017a0:	e8 3b f9 ff ff       	call   4010e0 <select@plt>
  4017a5:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
  4017aa:	74 0e                	je     4017ba <__rt_wait+0x15a>
  4017ac:	c6 05 fd 28 00 00 01 	movb   $0x1,0x28fd(%rip)        # 4040b0 <__stdin_ready>
  4017b3:	c6 05 f7 28 00 00 01 	movb   $0x1,0x28f7(%rip)        # 4040b1 <__io_pending>
  4017ba:	c6 05 f0 28 00 00 01 	movb   $0x1,0x28f0(%rip)        # 4040b1 <__io_pending>
  4017c1:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  4017c8:	c3                   	ret
  4017c9:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
  4017ce:	0f 84 15 ff ff ff    	je     4016e9 <__rt_wait+0x89>
  4017d4:	c6 05 d5 28 00 00 01 	movb   $0x1,0x28d5(%rip)        # 4040b0 <__stdin_ready>
  4017db:	c6 05 cf 28 00 00 01 	movb   $0x1,0x28cf(%rip)        # 4040b1 <__io_pending>
  4017e2:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  4017e9:	c3                   	ret
  4017ea:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

00000000004017f0 <__rt_poll>:
  4017f0:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
  4017f7:	8b 3d ab 28 00 00    	mov    0x28ab(%rip),%edi        # 4040a8 <g_epoll_fd>
  4017fd:	85 ff                	test   %edi,%edi
  4017ff:	79 3f                	jns    401840 <__rt_poll+0x50>
  401801:	31 ff                	xor    %edi,%edi
  401803:	e8 48 f9 ff ff       	call   401150 <epoll_create1@plt>
  401808:	89 05 9a 28 00 00    	mov    %eax,0x289a(%rip)        # 4040a8 <g_epoll_fd>
  40180e:	85 c0                	test   %eax,%eax
  401810:	0f 88 d5 00 00 00    	js     4018eb <__rt_poll+0xfb>
  401816:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
  40181d:	00 
  40181e:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
  401825:	00 00 
  401827:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
  40182c:	89 c7                	mov    %eax,%edi
  40182e:	be 01 00 00 00       	mov    $0x1,%esi
  401833:	31 d2                	xor    %edx,%edx
  401835:	e8 56 f8 ff ff       	call   401090 <epoll_ctl@plt>
  40183a:	8b 3d 68 28 00 00    	mov    0x2868(%rip),%edi        # 4040a8 <g_epoll_fd>
  401840:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  401845:	ba 40 00 00 00       	mov    $0x40,%edx
  40184a:	31 c9                	xor    %ecx,%ecx
  40184c:	e8 af f8 ff ff       	call   401100 <epoll_wait@plt>
  401851:	85 c0                	test   %eax,%eax
  401853:	7e 1d                	jle    401872 <__rt_poll+0x82>
  401855:	89 c1                	mov    %eax,%ecx
  401857:	83 f8 01             	cmp    $0x1,%eax
  40185a:	75 25                	jne    401881 <__rt_poll+0x91>
  40185c:	31 c0                	xor    %eax,%eax
  40185e:	f6 c1 01             	test   $0x1,%cl
  401861:	74 0f                	je     401872 <__rt_poll+0x82>
  401863:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
  401867:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
  40186c:	0f 84 cc 00 00 00    	je     40193e <__rt_poll+0x14e>
  401872:	c6 05 38 28 00 00 01 	movb   $0x1,0x2838(%rip)        # 4040b1 <__io_pending>
  401879:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  401880:	c3                   	ret
  401881:	89 c8                	mov    %ecx,%eax
  401883:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
  401888:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
  40188d:	48 89 c6             	mov    %rax,%rsi
  401890:	eb 18                	jmp    4018aa <__rt_poll+0xba>
  401892:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401899:	1f 84 00 00 00 00 00 
  4018a0:	48 83 c2 18          	add    $0x18,%rdx
  4018a4:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
  4018a8:	74 b4                	je     40185e <__rt_poll+0x6e>
  4018aa:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
  4018ae:	75 20                	jne    4018d0 <__rt_poll+0xe0>
  4018b0:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
  4018b4:	74 1a                	je     4018d0 <__rt_poll+0xe0>
  4018b6:	c6 05 f3 27 00 00 01 	movb   $0x1,0x27f3(%rip)        # 4040b0 <__stdin_ready>
  4018bd:	c6 05 ed 27 00 00 01 	movb   $0x1,0x27ed(%rip)        # 4040b1 <__io_pending>
  4018c4:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
  4018cb:	00 00 00 00 00 
  4018d0:	83 3a 00             	cmpl   $0x0,(%rdx)
  4018d3:	75 cb                	jne    4018a0 <__rt_poll+0xb0>
  4018d5:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
  4018d9:	74 c5                	je     4018a0 <__rt_poll+0xb0>
  4018db:	c6 05 ce 27 00 00 01 	movb   $0x1,0x27ce(%rip)        # 4040b0 <__stdin_ready>
  4018e2:	c6 05 c8 27 00 00 01 	movb   $0x1,0x27c8(%rip)        # 4040b1 <__io_pending>
  4018e9:	eb b5                	jmp    4018a0 <__rt_poll+0xb0>
  4018eb:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4018ef:	c5 fc 11 44 24 30    	vmovups %ymm0,0x30(%rsp)
  4018f5:	c5 fc 11 44 24 50    	vmovups %ymm0,0x50(%rsp)
  4018fb:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  401901:	c5 f8 28 05 57 07 00 	vmovaps 0x757(%rip),%xmm0        # 402060 <_IO_stdin_used+0x60>
  401908:	00 
  401909:	c5 fc 11 44 24 10    	vmovups %ymm0,0x10(%rsp)
  40190f:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  401913:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
  401918:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  40191d:	49 89 e0             	mov    %rsp,%r8
  401920:	bf 01 00 00 00       	mov    $0x1,%edi
  401925:	31 d2                	xor    %edx,%edx
  401927:	31 c9                	xor    %ecx,%ecx
  401929:	c5 f8 77             	vzeroupper
  40192c:	e8 af f7 ff ff       	call   4010e0 <select@plt>
  401931:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
  401936:	0f 84 36 ff ff ff    	je     401872 <__rt_poll+0x82>
  40193c:	eb 0b                	jmp    401949 <__rt_poll+0x159>
  40193e:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
  401943:	0f 84 29 ff ff ff    	je     401872 <__rt_poll+0x82>
  401949:	c6 05 60 27 00 00 01 	movb   $0x1,0x2760(%rip)        # 4040b0 <__stdin_ready>
  401950:	c6 05 5a 27 00 00 01 	movb   $0x1,0x275a(%rip)        # 4040b1 <__io_pending>
  401957:	c6 05 53 27 00 00 01 	movb   $0x1,0x2753(%rip)        # 4040b1 <__io_pending>
  40195e:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  401965:	c3                   	ret
  401966:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  40196d:	00 00 00 

0000000000401970 <__wait_for_event>:
  401970:	e9 eb fc ff ff       	jmp    401660 <__rt_wait>
  401975:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  40197c:	00 00 00 00 

0000000000401980 <__print>:
  401980:	50                   	push   %rax
  401981:	48 8b 05 40 26 00 00 	mov    0x2640(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401988:	48 8b 30             	mov    (%rax),%rsi
  40198b:	e8 f0 f6 ff ff       	call   401080 <fputs@plt>
  401990:	b8 01 00 00 00       	mov    $0x1,%eax
  401995:	59                   	pop    %rcx
  401996:	c3                   	ret
  401997:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  40199e:	00 00 

00000000004019a0 <__print_int>:
  4019a0:	50                   	push   %rax
  4019a1:	48 89 fa             	mov    %rdi,%rdx
  4019a4:	48 8b 05 35 26 00 00 	mov    0x2635(%rip),%rax        # 403fe0 <stderr@GLIBC_2.2.5>
  4019ab:	48 8b 38             	mov    (%rax),%rdi
  4019ae:	be a8 20 40 00       	mov    $0x4020a8,%esi
  4019b3:	31 c0                	xor    %eax,%eax
  4019b5:	e8 06 f7 ff ff       	call   4010c0 <fprintf@plt>
  4019ba:	b8 01 00 00 00       	mov    $0x1,%eax
  4019bf:	59                   	pop    %rcx
  4019c0:	c3                   	ret
  4019c1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4019c8:	0f 1f 84 00 00 00 00 
  4019cf:	00 

00000000004019d0 <__print_float>:
  4019d0:	50                   	push   %rax
  4019d1:	48 8b 05 08 26 00 00 	mov    0x2608(%rip),%rax        # 403fe0 <stderr@GLIBC_2.2.5>
  4019d8:	48 8b 38             	mov    (%rax),%rdi
  4019db:	c5 fa 5a c0          	vcvtss2sd %xmm0,%xmm0,%xmm0
  4019df:	be ae 20 40 00       	mov    $0x4020ae,%esi
  4019e4:	b0 01                	mov    $0x1,%al
  4019e6:	e8 d5 f6 ff ff       	call   4010c0 <fprintf@plt>
  4019eb:	b8 01 00 00 00       	mov    $0x1,%eax
  4019f0:	59                   	pop    %rcx
  4019f1:	c3                   	ret
  4019f2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4019f9:	1f 84 00 00 00 00 00 

0000000000401a00 <__sqrtf>:
  401a00:	e9 eb f6 ff ff       	jmp    4010f0 <sqrtf@plt>
  401a05:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  401a0c:	00 00 00 00 

0000000000401a10 <__exit>:
  401a10:	50                   	push   %rax
  401a11:	31 ff                	xor    %edi,%edi
  401a13:	e8 18 f7 ff ff       	call   401130 <exit@plt>
  401a18:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  401a1f:	00 

0000000000401a20 <__print_str_len>:
  401a20:	53                   	push   %rbx
  401a21:	48 89 f3             	mov    %rsi,%rbx
  401a24:	48 8b 05 9d 25 00 00 	mov    0x259d(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401a2b:	48 8b 08             	mov    (%rax),%rcx
  401a2e:	be 01 00 00 00       	mov    $0x1,%esi
  401a33:	48 89 da             	mov    %rbx,%rdx
  401a36:	e8 05 f7 ff ff       	call   401140 <fwrite@plt>
  401a3b:	48 89 d8             	mov    %rbx,%rax
  401a3e:	5b                   	pop    %rbx
  401a3f:	c3                   	ret

0000000000401a40 <__write_bytes>:
  401a40:	53                   	push   %rbx
  401a41:	48 89 f3             	mov    %rsi,%rbx
  401a44:	48 8b 05 7d 25 00 00 	mov    0x257d(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401a4b:	48 8b 08             	mov    (%rax),%rcx
  401a4e:	be 01 00 00 00       	mov    $0x1,%esi
  401a53:	48 89 da             	mov    %rbx,%rdx
  401a56:	e8 e5 f6 ff ff       	call   401140 <fwrite@plt>
  401a5b:	48 89 d8             	mov    %rbx,%rax
  401a5e:	5b                   	pop    %rbx
  401a5f:	c3                   	ret

0000000000401a60 <__read_stdin>:
  401a60:	48 89 f2             	mov    %rsi,%rdx
  401a63:	48 8b 05 66 25 00 00 	mov    0x2566(%rip),%rax        # 403fd0 <stdin@GLIBC_2.2.5>
  401a6a:	48 8b 08             	mov    (%rax),%rcx
  401a6d:	be 01 00 00 00       	mov    $0x1,%esi
  401a72:	e9 e9 f5 ff ff       	jmp    401060 <fread@plt>
  401a77:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  401a7e:	00 00 

0000000000401a80 <__putchar>:
  401a80:	53                   	push   %rbx
  401a81:	48 89 fb             	mov    %rdi,%rbx
  401a84:	48 8b 05 3d 25 00 00 	mov    0x253d(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401a8b:	48 8b 30             	mov    (%rax),%rsi
  401a8e:	e8 0d f6 ff ff       	call   4010a0 <putc@plt>
  401a93:	48 89 d8             	mov    %rbx,%rax
  401a96:	5b                   	pop    %rbx
  401a97:	c3                   	ret
  401a98:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  401a9f:	00 

0000000000401aa0 <brief_thread_pool_init>:
  401aa0:	c3                   	ret
  401aa1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401aa8:	0f 1f 84 00 00 00 00 
  401aaf:	00 

0000000000401ab0 <brief_barrier_release>:
  401ab0:	c3                   	ret
  401ab1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401ab8:	0f 1f 84 00 00 00 00 
  401abf:	00 

0000000000401ac0 <brief_barrier_wait>:
  401ac0:	c3                   	ret
  401ac1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401ac8:	0f 1f 84 00 00 00 00 
  401acf:	00 

0000000000401ad0 <brief_thread_pool_shutdown>:
  401ad0:	c3                   	ret

Disassembly of section .fini:

0000000000401ad4 <_fini>:
  401ad4:	f3 0f 1e fa          	endbr64
  401ad8:	48 83 ec 08          	sub    $0x8,%rsp
  401adc:	48 83 c4 08          	add    $0x8,%rsp
  401ae0:	c3                   	ret
