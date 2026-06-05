
benchmarks/cancel_math:     file format elf64-x86-64


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

00000000004010a0 <signal@plt>:
  4010a0:	ff 25 92 2f 00 00    	jmp    *0x2f92(%rip)        # 404038 <signal@GLIBC_2.2.5>
  4010a6:	68 07 00 00 00       	push   $0x7
  4010ab:	e9 70 ff ff ff       	jmp    401020 <_init+0x20>

00000000004010b0 <fprintf@plt>:
  4010b0:	ff 25 8a 2f 00 00    	jmp    *0x2f8a(%rip)        # 404040 <fprintf@GLIBC_2.2.5>
  4010b6:	68 08 00 00 00       	push   $0x8
  4010bb:	e9 60 ff ff ff       	jmp    401020 <_init+0x20>

00000000004010c0 <strtol@plt>:
  4010c0:	ff 25 82 2f 00 00    	jmp    *0x2f82(%rip)        # 404048 <strtol@GLIBC_2.2.5>
  4010c6:	68 09 00 00 00       	push   $0x9
  4010cb:	e9 50 ff ff ff       	jmp    401020 <_init+0x20>

00000000004010d0 <select@plt>:
  4010d0:	ff 25 7a 2f 00 00    	jmp    *0x2f7a(%rip)        # 404050 <select@GLIBC_2.2.5>
  4010d6:	68 0a 00 00 00       	push   $0xa
  4010db:	e9 40 ff ff ff       	jmp    401020 <_init+0x20>

00000000004010e0 <sqrtf@plt>:
  4010e0:	ff 25 72 2f 00 00    	jmp    *0x2f72(%rip)        # 404058 <sqrtf@GLIBC_2.2.5>
  4010e6:	68 0b 00 00 00       	push   $0xb
  4010eb:	e9 30 ff ff ff       	jmp    401020 <_init+0x20>

00000000004010f0 <epoll_wait@plt>:
  4010f0:	ff 25 6a 2f 00 00    	jmp    *0x2f6a(%rip)        # 404060 <epoll_wait@GLIBC_2.3.2>
  4010f6:	68 0c 00 00 00       	push   $0xc
  4010fb:	e9 20 ff ff ff       	jmp    401020 <_init+0x20>

0000000000401100 <setvbuf@plt>:
  401100:	ff 25 62 2f 00 00    	jmp    *0x2f62(%rip)        # 404068 <setvbuf@GLIBC_2.2.5>
  401106:	68 0d 00 00 00       	push   $0xd
  40110b:	e9 10 ff ff ff       	jmp    401020 <_init+0x20>

0000000000401110 <__libc_current_sigrtmin@plt>:
  401110:	ff 25 5a 2f 00 00    	jmp    *0x2f5a(%rip)        # 404070 <__libc_current_sigrtmin@GLIBC_2.2.5>
  401116:	68 0e 00 00 00       	push   $0xe
  40111b:	e9 00 ff ff ff       	jmp    401020 <_init+0x20>

0000000000401120 <exit@plt>:
  401120:	ff 25 52 2f 00 00    	jmp    *0x2f52(%rip)        # 404078 <exit@GLIBC_2.2.5>
  401126:	68 0f 00 00 00       	push   $0xf
  40112b:	e9 f0 fe ff ff       	jmp    401020 <_init+0x20>

0000000000401130 <epoll_create1@plt>:
  401130:	ff 25 4a 2f 00 00    	jmp    *0x2f4a(%rip)        # 404080 <epoll_create1@GLIBC_2.9>
  401136:	68 10 00 00 00       	push   $0x10
  40113b:	e9 e0 fe ff ff       	jmp    401020 <_init+0x20>

Disassembly of section .text:

0000000000401140 <_start>:
  401140:	f3 0f 1e fa          	endbr64
  401144:	31 ed                	xor    %ebp,%ebp
  401146:	49 89 d1             	mov    %rdx,%r9
  401149:	5e                   	pop    %rsi
  40114a:	48 89 e2             	mov    %rsp,%rdx
  40114d:	48 83 e4 f0          	and    $0xfffffffffffffff0,%rsp
  401151:	50                   	push   %rax
  401152:	54                   	push   %rsp
  401153:	45 31 c0             	xor    %r8d,%r8d
  401156:	31 c9                	xor    %ecx,%ecx
  401158:	48 c7 c7 a0 12 40 00 	mov    $0x4012a0,%rdi
  40115f:	ff 15 5b 2e 00 00    	call   *0x2e5b(%rip)        # 403fc0 <__libc_start_main@GLIBC_2.34>
  401165:	f4                   	hlt
  401166:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  40116d:	00 00 00 

0000000000401170 <_dl_relocate_static_pie>:
  401170:	f3 0f 1e fa          	endbr64
  401174:	c3                   	ret
  401175:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  40117c:	00 00 00 
  40117f:	90                   	nop

0000000000401180 <deregister_tm_clones>:
  401180:	b8 a0 40 40 00       	mov    $0x4040a0,%eax
  401185:	48 3d a0 40 40 00    	cmp    $0x4040a0,%rax
  40118b:	74 13                	je     4011a0 <deregister_tm_clones+0x20>
  40118d:	b8 00 00 00 00       	mov    $0x0,%eax
  401192:	48 85 c0             	test   %rax,%rax
  401195:	74 09                	je     4011a0 <deregister_tm_clones+0x20>
  401197:	bf a0 40 40 00       	mov    $0x4040a0,%edi
  40119c:	ff e0                	jmp    *%rax
  40119e:	66 90                	xchg   %ax,%ax
  4011a0:	c3                   	ret
  4011a1:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  4011a8:	00 00 00 00 
  4011ac:	0f 1f 40 00          	nopl   0x0(%rax)

00000000004011b0 <register_tm_clones>:
  4011b0:	be a0 40 40 00       	mov    $0x4040a0,%esi
  4011b5:	48 81 ee a0 40 40 00 	sub    $0x4040a0,%rsi
  4011bc:	48 89 f0             	mov    %rsi,%rax
  4011bf:	48 c1 ee 3f          	shr    $0x3f,%rsi
  4011c3:	48 c1 f8 03          	sar    $0x3,%rax
  4011c7:	48 01 c6             	add    %rax,%rsi
  4011ca:	48 d1 fe             	sar    $1,%rsi
  4011cd:	74 11                	je     4011e0 <register_tm_clones+0x30>
  4011cf:	b8 00 00 00 00       	mov    $0x0,%eax
  4011d4:	48 85 c0             	test   %rax,%rax
  4011d7:	74 07                	je     4011e0 <register_tm_clones+0x30>
  4011d9:	bf a0 40 40 00       	mov    $0x4040a0,%edi
  4011de:	ff e0                	jmp    *%rax
  4011e0:	c3                   	ret
  4011e1:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  4011e8:	00 00 00 00 
  4011ec:	0f 1f 40 00          	nopl   0x0(%rax)

00000000004011f0 <__do_global_dtors_aux>:
  4011f0:	f3 0f 1e fa          	endbr64
  4011f4:	80 3d c5 2e 00 00 00 	cmpb   $0x0,0x2ec5(%rip)        # 4040c0 <completed.0>
  4011fb:	75 13                	jne    401210 <__do_global_dtors_aux+0x20>
  4011fd:	55                   	push   %rbp
  4011fe:	48 89 e5             	mov    %rsp,%rbp
  401201:	e8 7a ff ff ff       	call   401180 <deregister_tm_clones>
  401206:	c6 05 b3 2e 00 00 01 	movb   $0x1,0x2eb3(%rip)        # 4040c0 <completed.0>
  40120d:	5d                   	pop    %rbp
  40120e:	c3                   	ret
  40120f:	90                   	nop
  401210:	c3                   	ret
  401211:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  401218:	00 00 00 00 
  40121c:	0f 1f 40 00          	nopl   0x0(%rax)

0000000000401220 <frame_dummy>:
  401220:	f3 0f 1e fa          	endbr64
  401224:	eb 8a                	jmp    4011b0 <register_tm_clones>
  401226:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  40122d:	00 00 00 

0000000000401230 <step>:
  401230:	48 89 f8             	mov    %rdi,%rax
  401233:	48 8b 4f 08          	mov    0x8(%rdi),%rcx
  401237:	48 8b 7f 10          	mov    0x10(%rdi),%rdi
  40123b:	48 01 cf             	add    %rcx,%rdi
  40123e:	48 89 78 10          	mov    %rdi,0x10(%rax)
  401242:	48 ff c1             	inc    %rcx
  401245:	48 89 48 08          	mov    %rcx,0x8(%rax)
  401249:	48 b8 a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%rax
  401250:	24 03 e5 
  401253:	48 0f af c1          	imul   %rcx,%rax
  401257:	48 b9 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%rcx
  40125e:	6b 00 00 
  401261:	48 01 c1             	add    %rax,%rcx
  401264:	48 0f ac c9 06       	shrd   $0x6,%rcx,%rcx
  401269:	48 b8 94 57 53 fe 5a 	movabs $0x35afe535794,%rax
  401270:	03 00 00 
  401273:	48 39 c1             	cmp    %rax,%rcx
  401276:	0f 86 d4 06 00 00    	jbe    401950 <__print_int>
  40127c:	c3                   	ret
  40127d:	0f 1f 00             	nopl   (%rax)

0000000000401280 <init_state>:
  401280:	53                   	push   %rbx
  401281:	48 89 fb             	mov    %rdi,%rbx
  401284:	bf 98 20 40 00       	mov    $0x402098,%edi
  401289:	e8 32 03 00 00       	call   4015c0 <__get_env_int>
  40128e:	48 89 03             	mov    %rax,(%rbx)
  401291:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  401295:	c5 f8 11 43 08       	vmovups %xmm0,0x8(%rbx)
  40129a:	5b                   	pop    %rbx
  40129b:	c3                   	ret
  40129c:	0f 1f 40 00          	nopl   0x0(%rax)

00000000004012a0 <main>:
  4012a0:	55                   	push   %rbp
  4012a1:	41 57                	push   %r15
  4012a3:	41 56                	push   %r14
  4012a5:	41 55                	push   %r13
  4012a7:	41 54                	push   %r12
  4012a9:	53                   	push   %rbx
  4012aa:	48 83 ec 18          	sub    $0x18,%rsp
  4012ae:	bf 98 20 40 00       	mov    $0x402098,%edi
  4012b3:	e8 08 03 00 00       	call   4015c0 <__get_env_int>
  4012b8:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4012bd:	48 83 c0 fd          	add    $0xfffffffffffffffd,%rax
  4012c1:	48 89 44 24 10       	mov    %rax,0x10(%rsp)
  4012c6:	45 31 ff             	xor    %r15d,%r15d
  4012c9:	49 bc a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%r12
  4012d0:	24 03 e5 
  4012d3:	49 bd 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%r13
  4012da:	6b 00 00 
  4012dd:	48 bd 94 57 53 fe 5a 	movabs $0x35afe535794,%rbp
  4012e4:	03 00 00 
  4012e7:	31 db                	xor    %ebx,%ebx
  4012e9:	eb 0d                	jmp    4012f8 <main+0x58>
  4012eb:	0f 1f 44 00 00       	nopl   0x0(%rax,%rax,1)
  4012f0:	4c 89 f7             	mov    %r14,%rdi
  4012f3:	e8 58 06 00 00       	call   401950 <__print_int>
  4012f8:	4d 89 fe             	mov    %r15,%r14
  4012fb:	48 3b 5c 24 10       	cmp    0x10(%rsp),%rbx
  401300:	0f 8d 9a 00 00 00    	jge    4013a0 <main+0x100>
  401306:	48 89 d8             	mov    %rbx,%rax
  401309:	49 0f af c4          	imul   %r12,%rax
  40130d:	4c 01 e8             	add    %r13,%rax
  401310:	48 0f ac c0 06       	shrd   $0x6,%rax,%rax
  401315:	48 39 e8             	cmp    %rbp,%rax
  401318:	77 08                	ja     401322 <main+0x82>
  40131a:	4c 89 f7             	mov    %r14,%rdi
  40131d:	e8 2e 06 00 00       	call   401950 <__print_int>
  401322:	49 01 de             	add    %rbx,%r14
  401325:	4c 8d 7b 01          	lea    0x1(%rbx),%r15
  401329:	4c 89 f8             	mov    %r15,%rax
  40132c:	49 0f af c4          	imul   %r12,%rax
  401330:	4c 01 e8             	add    %r13,%rax
  401333:	48 0f ac c0 06       	shrd   $0x6,%rax,%rax
  401338:	48 39 e8             	cmp    %rbp,%rax
  40133b:	77 08                	ja     401345 <main+0xa5>
  40133d:	4c 89 f7             	mov    %r14,%rdi
  401340:	e8 0b 06 00 00       	call   401950 <__print_int>
  401345:	4d 01 fe             	add    %r15,%r14
  401348:	4c 8d 7b 02          	lea    0x2(%rbx),%r15
  40134c:	4c 89 f8             	mov    %r15,%rax
  40134f:	49 0f af c4          	imul   %r12,%rax
  401353:	4c 01 e8             	add    %r13,%rax
  401356:	48 0f ac c0 06       	shrd   $0x6,%rax,%rax
  40135b:	48 39 e8             	cmp    %rbp,%rax
  40135e:	77 08                	ja     401368 <main+0xc8>
  401360:	4c 89 f7             	mov    %r14,%rdi
  401363:	e8 e8 05 00 00       	call   401950 <__print_int>
  401368:	4d 01 fe             	add    %r15,%r14
  40136b:	48 8d 43 03          	lea    0x3(%rbx),%rax
  40136f:	4d 8d 3c 06          	lea    (%r14,%rax,1),%r15
  401373:	48 83 c3 04          	add    $0x4,%rbx
  401377:	49 0f af c4          	imul   %r12,%rax
  40137b:	4c 01 e8             	add    %r13,%rax
  40137e:	48 0f ac c0 06       	shrd   $0x6,%rax,%rax
  401383:	48 39 e8             	cmp    %rbp,%rax
  401386:	0f 87 6c ff ff ff    	ja     4012f8 <main+0x58>
  40138c:	e9 5f ff ff ff       	jmp    4012f0 <main+0x50>
  401391:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401398:	0f 1f 84 00 00 00 00 
  40139f:	00 
  4013a0:	48 3b 5c 24 08       	cmp    0x8(%rsp),%rbx
  4013a5:	7d 2f                	jge    4013d6 <main+0x136>
  4013a7:	4e 8d 3c 33          	lea    (%rbx,%r14,1),%r15
  4013ab:	48 8d 43 01          	lea    0x1(%rbx),%rax
  4013af:	49 0f af dc          	imul   %r12,%rbx
  4013b3:	4c 01 eb             	add    %r13,%rbx
  4013b6:	48 0f ac db 06       	shrd   $0x6,%rbx,%rbx
  4013bb:	48 b9 95 57 53 fe 5a 	movabs $0x35afe535795,%rcx
  4013c2:	03 00 00 
  4013c5:	48 39 cb             	cmp    %rcx,%rbx
  4013c8:	48 89 c3             	mov    %rax,%rbx
  4013cb:	0f 83 27 ff ff ff    	jae    4012f8 <main+0x58>
  4013d1:	e9 1a ff ff ff       	jmp    4012f0 <main+0x50>
  4013d6:	31 c0                	xor    %eax,%eax
  4013d8:	48 83 c4 18          	add    $0x18,%rsp
  4013dc:	5b                   	pop    %rbx
  4013dd:	41 5c                	pop    %r12
  4013df:	41 5d                	pop    %r13
  4013e1:	41 5e                	pop    %r14
  4013e3:	41 5f                	pop    %r15
  4013e5:	5d                   	pop    %rbp
  4013e6:	c3                   	ret
  4013e7:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  4013ee:	00 00 

00000000004013f0 <brief_rt_ctor>:
  4013f0:	e9 0b 00 00 00       	jmp    401400 <__rt_init>
  4013f5:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  4013fc:	00 00 00 00 

0000000000401400 <__rt_init>:
  401400:	53                   	push   %rbx
  401401:	48 81 ec 00 01 00 00 	sub    $0x100,%rsp
  401408:	be 80 15 40 00       	mov    $0x401580,%esi
  40140d:	bf 02 00 00 00       	mov    $0x2,%edi
  401412:	e8 89 fc ff ff       	call   4010a0 <signal@plt>
  401417:	be 90 15 40 00       	mov    $0x401590,%esi
  40141c:	bf 0f 00 00 00       	mov    $0xf,%edi
  401421:	e8 7a fc ff ff       	call   4010a0 <signal@plt>
  401426:	be a0 15 40 00       	mov    $0x4015a0,%esi
  40142b:	bf 01 00 00 00       	mov    $0x1,%edi
  401430:	e8 6b fc ff ff       	call   4010a0 <signal@plt>
  401435:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  401439:	c5 fc 11 84 24 e0 00 	vmovups %ymm0,0xe0(%rsp)
  401440:	00 00 
  401442:	c5 fc 11 84 24 d0 00 	vmovups %ymm0,0xd0(%rsp)
  401449:	00 00 
  40144b:	c5 fc 11 84 24 b0 00 	vmovups %ymm0,0xb0(%rsp)
  401452:	00 00 
  401454:	c5 fc 11 84 24 90 00 	vmovups %ymm0,0x90(%rsp)
  40145b:	00 00 
  40145d:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  401463:	48 c7 44 24 68 b0 15 	movq   $0x4015b0,0x68(%rsp)
  40146a:	40 00 
  40146c:	c7 84 24 f0 00 00 00 	movl   $0x4,0xf0(%rsp)
  401473:	04 00 00 00 
  401477:	c5 f8 77             	vzeroupper
  40147a:	e8 91 fc ff ff       	call   401110 <__libc_current_sigrtmin@plt>
  40147f:	8d 78 01             	lea    0x1(%rax),%edi
  401482:	48 8d 5c 24 68       	lea    0x68(%rsp),%rbx
  401487:	48 89 de             	mov    %rbx,%rsi
  40148a:	31 d2                	xor    %edx,%edx
  40148c:	e8 bf fb ff ff       	call   401050 <sigaction@plt>
  401491:	e8 7a fc ff ff       	call   401110 <__libc_current_sigrtmin@plt>
  401496:	8d 78 02             	lea    0x2(%rax),%edi
  401499:	48 89 de             	mov    %rbx,%rsi
  40149c:	31 d2                	xor    %edx,%edx
  40149e:	e8 ad fb ff ff       	call   401050 <sigaction@plt>
  4014a3:	e8 68 fc ff ff       	call   401110 <__libc_current_sigrtmin@plt>
  4014a8:	ff c0                	inc    %eax
  4014aa:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4014ae:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
  4014b3:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
  4014b9:	89 44 24 08          	mov    %eax,0x8(%rsp)
  4014bd:	48 89 e6             	mov    %rsp,%rsi
  4014c0:	ba c8 40 40 00       	mov    $0x4040c8,%edx
  4014c5:	31 ff                	xor    %edi,%edi
  4014c7:	c5 f8 77             	vzeroupper
  4014ca:	e8 a1 fb ff ff       	call   401070 <timer_create@plt>
  4014cf:	85 c0                	test   %eax,%eax
  4014d1:	75 27                	jne    4014fa <__rt_init+0xfa>
  4014d3:	c4 e2 7d 1a 05 84 0b 	vbroadcastf128 0xb84(%rip),%ymm0        # 402060 <_IO_stdin_used+0x60>
  4014da:	00 00 
  4014dc:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
  4014e2:	48 8b 3d df 2b 00 00 	mov    0x2bdf(%rip),%rdi        # 4040c8 <g_timer_1hz>
  4014e9:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
  4014ee:	31 f6                	xor    %esi,%esi
  4014f0:	31 c9                	xor    %ecx,%ecx
  4014f2:	c5 f8 77             	vzeroupper
  4014f5:	e8 46 fb ff ff       	call   401040 <timer_settime@plt>
  4014fa:	e8 11 fc ff ff       	call   401110 <__libc_current_sigrtmin@plt>
  4014ff:	83 c0 02             	add    $0x2,%eax
  401502:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  401506:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
  40150b:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
  401511:	89 44 24 08          	mov    %eax,0x8(%rsp)
  401515:	48 89 e6             	mov    %rsp,%rsi
  401518:	ba d0 40 40 00       	mov    $0x4040d0,%edx
  40151d:	31 ff                	xor    %edi,%edi
  40151f:	c5 f8 77             	vzeroupper
  401522:	e8 49 fb ff ff       	call   401070 <timer_create@plt>
  401527:	85 c0                	test   %eax,%eax
  401529:	75 27                	jne    401552 <__rt_init+0x152>
  40152b:	c4 e2 7d 1a 05 3c 0b 	vbroadcastf128 0xb3c(%rip),%ymm0        # 402070 <_IO_stdin_used+0x70>
  401532:	00 00 
  401534:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
  40153a:	48 8b 3d 8f 2b 00 00 	mov    0x2b8f(%rip),%rdi        # 4040d0 <g_timer_100hz>
  401541:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
  401546:	31 f6                	xor    %esi,%esi
  401548:	31 c9                	xor    %ecx,%ecx
  40154a:	c5 f8 77             	vzeroupper
  40154d:	e8 ee fa ff ff       	call   401040 <timer_settime@plt>
  401552:	48 8b 05 6f 2a 00 00 	mov    0x2a6f(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401559:	48 8b 38             	mov    (%rax),%rdi
  40155c:	31 f6                	xor    %esi,%esi
  40155e:	ba 01 00 00 00       	mov    $0x1,%edx
  401563:	31 c9                	xor    %ecx,%ecx
  401565:	e8 96 fb ff ff       	call   401100 <setvbuf@plt>
  40156a:	c6 05 30 2b 00 00 01 	movb   $0x1,0x2b30(%rip)        # 4040a1 <__io_pending>
  401571:	48 81 c4 00 01 00 00 	add    $0x100,%rsp
  401578:	5b                   	pop    %rbx
  401579:	c3                   	ret
  40157a:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

0000000000401580 <handle_sigint>:
  401580:	c6 05 1b 2b 00 00 01 	movb   $0x1,0x2b1b(%rip)        # 4040a2 <__sigint_flag>
  401587:	c6 05 13 2b 00 00 01 	movb   $0x1,0x2b13(%rip)        # 4040a1 <__io_pending>
  40158e:	c3                   	ret
  40158f:	90                   	nop

0000000000401590 <handle_sigterm>:
  401590:	c6 05 0c 2b 00 00 01 	movb   $0x1,0x2b0c(%rip)        # 4040a3 <__sigterm_flag>
  401597:	c6 05 03 2b 00 00 01 	movb   $0x1,0x2b03(%rip)        # 4040a1 <__io_pending>
  40159e:	c3                   	ret
  40159f:	90                   	nop

00000000004015a0 <handle_sighup>:
  4015a0:	c6 05 fd 2a 00 00 01 	movb   $0x1,0x2afd(%rip)        # 4040a4 <__sighup_flag>
  4015a7:	c6 05 f3 2a 00 00 01 	movb   $0x1,0x2af3(%rip)        # 4040a1 <__io_pending>
  4015ae:	c3                   	ret
  4015af:	90                   	nop

00000000004015b0 <handle_timer>:
  4015b0:	48 ff 05 f1 2a 00 00 	incq   0x2af1(%rip)        # 4040a8 <__timer_1hz>
  4015b7:	c6 05 e3 2a 00 00 01 	movb   $0x1,0x2ae3(%rip)        # 4040a1 <__io_pending>
  4015be:	c3                   	ret
  4015bf:	90                   	nop

00000000004015c0 <__get_env_int>:
  4015c0:	53                   	push   %rbx
  4015c1:	48 83 ec 10          	sub    $0x10,%rsp
  4015c5:	e8 66 fa ff ff       	call   401030 <getenv@plt>
  4015ca:	48 85 c0             	test   %rax,%rax
  4015cd:	74 32                	je     401601 <__get_env_int+0x41>
  4015cf:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
  4015d6:	00 00 
  4015d8:	48 8d 74 24 08       	lea    0x8(%rsp),%rsi
  4015dd:	48 89 c7             	mov    %rax,%rdi
  4015e0:	ba 0a 00 00 00       	mov    $0xa,%edx
  4015e5:	48 89 c3             	mov    %rax,%rbx
  4015e8:	e8 d3 fa ff ff       	call   4010c0 <strtol@plt>
  4015ed:	48 89 c1             	mov    %rax,%rcx
  4015f0:	31 c0                	xor    %eax,%eax
  4015f2:	48 39 5c 24 08       	cmp    %rbx,0x8(%rsp)
  4015f7:	48 0f 45 c1          	cmovne %rcx,%rax
  4015fb:	48 83 c4 10          	add    $0x10,%rsp
  4015ff:	5b                   	pop    %rbx
  401600:	c3                   	ret
  401601:	31 c0                	xor    %eax,%eax
  401603:	48 83 c4 10          	add    $0x10,%rsp
  401607:	5b                   	pop    %rbx
  401608:	c3                   	ret
  401609:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000401610 <__rt_wait>:
  401610:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
  401617:	8b 3d 7b 2a 00 00    	mov    0x2a7b(%rip),%edi        # 404098 <g_epoll_fd>
  40161d:	85 ff                	test   %edi,%edi
  40161f:	79 3f                	jns    401660 <__rt_wait+0x50>
  401621:	31 ff                	xor    %edi,%edi
  401623:	e8 08 fb ff ff       	call   401130 <epoll_create1@plt>
  401628:	89 05 6a 2a 00 00    	mov    %eax,0x2a6a(%rip)        # 404098 <g_epoll_fd>
  40162e:	85 c0                	test   %eax,%eax
  401630:	0f 88 d5 00 00 00    	js     40170b <__rt_wait+0xfb>
  401636:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
  40163d:	00 
  40163e:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
  401645:	00 00 
  401647:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
  40164c:	89 c7                	mov    %eax,%edi
  40164e:	be 01 00 00 00       	mov    $0x1,%esi
  401653:	31 d2                	xor    %edx,%edx
  401655:	e8 36 fa ff ff       	call   401090 <epoll_ctl@plt>
  40165a:	8b 3d 38 2a 00 00    	mov    0x2a38(%rip),%edi        # 404098 <g_epoll_fd>
  401660:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  401665:	ba 40 00 00 00       	mov    $0x40,%edx
  40166a:	b9 64 00 00 00       	mov    $0x64,%ecx
  40166f:	e8 7c fa ff ff       	call   4010f0 <epoll_wait@plt>
  401674:	85 c0                	test   %eax,%eax
  401676:	0f 8e ef 00 00 00    	jle    40176b <__rt_wait+0x15b>
  40167c:	89 c1                	mov    %eax,%ecx
  40167e:	83 f8 01             	cmp    $0x1,%eax
  401681:	75 1e                	jne    4016a1 <__rt_wait+0x91>
  401683:	31 c0                	xor    %eax,%eax
  401685:	f6 c1 01             	test   $0x1,%cl
  401688:	74 0f                	je     401699 <__rt_wait+0x89>
  40168a:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
  40168e:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
  401693:	0f 84 e1 00 00 00    	je     40177a <__rt_wait+0x16a>
  401699:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  4016a0:	c3                   	ret
  4016a1:	89 c8                	mov    %ecx,%eax
  4016a3:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
  4016a8:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
  4016ad:	48 89 c6             	mov    %rax,%rsi
  4016b0:	eb 18                	jmp    4016ca <__rt_wait+0xba>
  4016b2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4016b9:	1f 84 00 00 00 00 00 
  4016c0:	48 83 c2 18          	add    $0x18,%rdx
  4016c4:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
  4016c8:	74 bb                	je     401685 <__rt_wait+0x75>
  4016ca:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
  4016ce:	75 20                	jne    4016f0 <__rt_wait+0xe0>
  4016d0:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
  4016d4:	74 1a                	je     4016f0 <__rt_wait+0xe0>
  4016d6:	c6 05 c3 29 00 00 01 	movb   $0x1,0x29c3(%rip)        # 4040a0 <__stdin_ready>
  4016dd:	c6 05 bd 29 00 00 01 	movb   $0x1,0x29bd(%rip)        # 4040a1 <__io_pending>
  4016e4:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
  4016eb:	00 00 00 00 00 
  4016f0:	83 3a 00             	cmpl   $0x0,(%rdx)
  4016f3:	75 cb                	jne    4016c0 <__rt_wait+0xb0>
  4016f5:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
  4016f9:	74 c5                	je     4016c0 <__rt_wait+0xb0>
  4016fb:	c6 05 9e 29 00 00 01 	movb   $0x1,0x299e(%rip)        # 4040a0 <__stdin_ready>
  401702:	c6 05 98 29 00 00 01 	movb   $0x1,0x2998(%rip)        # 4040a1 <__io_pending>
  401709:	eb b5                	jmp    4016c0 <__rt_wait+0xb0>
  40170b:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  40170f:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  401715:	c5 fc 11 44 24 58    	vmovups %ymm0,0x58(%rsp)
  40171b:	c5 fc 11 44 24 38    	vmovups %ymm0,0x38(%rsp)
  401721:	c5 fc 11 44 24 18    	vmovups %ymm0,0x18(%rsp)
  401727:	48 c7 44 24 10 01 00 	movq   $0x1,0x10(%rsp)
  40172e:	00 00 
  401730:	c5 f8 10 05 48 09 00 	vmovups 0x948(%rip),%xmm0        # 402080 <_IO_stdin_used+0x80>
  401737:	00 
  401738:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
  40173d:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  401742:	49 89 e0             	mov    %rsp,%r8
  401745:	bf 01 00 00 00       	mov    $0x1,%edi
  40174a:	31 d2                	xor    %edx,%edx
  40174c:	31 c9                	xor    %ecx,%ecx
  40174e:	c5 f8 77             	vzeroupper
  401751:	e8 7a f9 ff ff       	call   4010d0 <select@plt>
  401756:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
  40175b:	74 0e                	je     40176b <__rt_wait+0x15b>
  40175d:	c6 05 3c 29 00 00 01 	movb   $0x1,0x293c(%rip)        # 4040a0 <__stdin_ready>
  401764:	c6 05 36 29 00 00 01 	movb   $0x1,0x2936(%rip)        # 4040a1 <__io_pending>
  40176b:	c6 05 2f 29 00 00 01 	movb   $0x1,0x292f(%rip)        # 4040a1 <__io_pending>
  401772:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  401779:	c3                   	ret
  40177a:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
  40177f:	0f 84 14 ff ff ff    	je     401699 <__rt_wait+0x89>
  401785:	c6 05 14 29 00 00 01 	movb   $0x1,0x2914(%rip)        # 4040a0 <__stdin_ready>
  40178c:	c6 05 0e 29 00 00 01 	movb   $0x1,0x290e(%rip)        # 4040a1 <__io_pending>
  401793:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  40179a:	c3                   	ret
  40179b:	0f 1f 44 00 00       	nopl   0x0(%rax,%rax,1)

00000000004017a0 <__rt_poll>:
  4017a0:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
  4017a7:	8b 3d eb 28 00 00    	mov    0x28eb(%rip),%edi        # 404098 <g_epoll_fd>
  4017ad:	85 ff                	test   %edi,%edi
  4017af:	79 3f                	jns    4017f0 <__rt_poll+0x50>
  4017b1:	31 ff                	xor    %edi,%edi
  4017b3:	e8 78 f9 ff ff       	call   401130 <epoll_create1@plt>
  4017b8:	89 05 da 28 00 00    	mov    %eax,0x28da(%rip)        # 404098 <g_epoll_fd>
  4017be:	85 c0                	test   %eax,%eax
  4017c0:	0f 88 d5 00 00 00    	js     40189b <__rt_poll+0xfb>
  4017c6:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
  4017cd:	00 
  4017ce:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
  4017d5:	00 00 
  4017d7:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
  4017dc:	89 c7                	mov    %eax,%edi
  4017de:	be 01 00 00 00       	mov    $0x1,%esi
  4017e3:	31 d2                	xor    %edx,%edx
  4017e5:	e8 a6 f8 ff ff       	call   401090 <epoll_ctl@plt>
  4017ea:	8b 3d a8 28 00 00    	mov    0x28a8(%rip),%edi        # 404098 <g_epoll_fd>
  4017f0:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  4017f5:	ba 40 00 00 00       	mov    $0x40,%edx
  4017fa:	31 c9                	xor    %ecx,%ecx
  4017fc:	e8 ef f8 ff ff       	call   4010f0 <epoll_wait@plt>
  401801:	85 c0                	test   %eax,%eax
  401803:	7e 1d                	jle    401822 <__rt_poll+0x82>
  401805:	89 c1                	mov    %eax,%ecx
  401807:	83 f8 01             	cmp    $0x1,%eax
  40180a:	75 25                	jne    401831 <__rt_poll+0x91>
  40180c:	31 c0                	xor    %eax,%eax
  40180e:	f6 c1 01             	test   $0x1,%cl
  401811:	74 0f                	je     401822 <__rt_poll+0x82>
  401813:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
  401817:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
  40181c:	0f 84 cd 00 00 00    	je     4018ef <__rt_poll+0x14f>
  401822:	c6 05 78 28 00 00 01 	movb   $0x1,0x2878(%rip)        # 4040a1 <__io_pending>
  401829:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  401830:	c3                   	ret
  401831:	89 c8                	mov    %ecx,%eax
  401833:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
  401838:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
  40183d:	48 89 c6             	mov    %rax,%rsi
  401840:	eb 18                	jmp    40185a <__rt_poll+0xba>
  401842:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401849:	1f 84 00 00 00 00 00 
  401850:	48 83 c2 18          	add    $0x18,%rdx
  401854:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
  401858:	74 b4                	je     40180e <__rt_poll+0x6e>
  40185a:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
  40185e:	75 20                	jne    401880 <__rt_poll+0xe0>
  401860:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
  401864:	74 1a                	je     401880 <__rt_poll+0xe0>
  401866:	c6 05 33 28 00 00 01 	movb   $0x1,0x2833(%rip)        # 4040a0 <__stdin_ready>
  40186d:	c6 05 2d 28 00 00 01 	movb   $0x1,0x282d(%rip)        # 4040a1 <__io_pending>
  401874:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
  40187b:	00 00 00 00 00 
  401880:	83 3a 00             	cmpl   $0x0,(%rdx)
  401883:	75 cb                	jne    401850 <__rt_poll+0xb0>
  401885:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
  401889:	74 c5                	je     401850 <__rt_poll+0xb0>
  40188b:	c6 05 0e 28 00 00 01 	movb   $0x1,0x280e(%rip)        # 4040a0 <__stdin_ready>
  401892:	c6 05 08 28 00 00 01 	movb   $0x1,0x2808(%rip)        # 4040a1 <__io_pending>
  401899:	eb b5                	jmp    401850 <__rt_poll+0xb0>
  40189b:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  40189f:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  4018a5:	c5 fc 11 44 24 58    	vmovups %ymm0,0x58(%rsp)
  4018ab:	c5 fc 11 44 24 38    	vmovups %ymm0,0x38(%rsp)
  4018b1:	c5 fc 11 44 24 18    	vmovups %ymm0,0x18(%rsp)
  4018b7:	48 c7 44 24 10 01 00 	movq   $0x1,0x10(%rsp)
  4018be:	00 00 
  4018c0:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4018c4:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
  4018c9:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  4018ce:	49 89 e0             	mov    %rsp,%r8
  4018d1:	bf 01 00 00 00       	mov    $0x1,%edi
  4018d6:	31 d2                	xor    %edx,%edx
  4018d8:	31 c9                	xor    %ecx,%ecx
  4018da:	c5 f8 77             	vzeroupper
  4018dd:	e8 ee f7 ff ff       	call   4010d0 <select@plt>
  4018e2:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
  4018e7:	0f 84 35 ff ff ff    	je     401822 <__rt_poll+0x82>
  4018ed:	eb 0b                	jmp    4018fa <__rt_poll+0x15a>
  4018ef:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
  4018f4:	0f 84 28 ff ff ff    	je     401822 <__rt_poll+0x82>
  4018fa:	c6 05 9f 27 00 00 01 	movb   $0x1,0x279f(%rip)        # 4040a0 <__stdin_ready>
  401901:	c6 05 99 27 00 00 01 	movb   $0x1,0x2799(%rip)        # 4040a1 <__io_pending>
  401908:	c6 05 92 27 00 00 01 	movb   $0x1,0x2792(%rip)        # 4040a1 <__io_pending>
  40190f:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  401916:	c3                   	ret
  401917:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  40191e:	00 00 

0000000000401920 <__wait_for_event>:
  401920:	e9 eb fc ff ff       	jmp    401610 <__rt_wait>
  401925:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  40192c:	00 00 00 00 

0000000000401930 <__print>:
  401930:	50                   	push   %rax
  401931:	48 8b 05 90 26 00 00 	mov    0x2690(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401938:	48 8b 30             	mov    (%rax),%rsi
  40193b:	e8 40 f7 ff ff       	call   401080 <fputs@plt>
  401940:	b8 01 00 00 00       	mov    $0x1,%eax
  401945:	59                   	pop    %rcx
  401946:	c3                   	ret
  401947:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  40194e:	00 00 

0000000000401950 <__print_int>:
  401950:	50                   	push   %rax
  401951:	48 89 fa             	mov    %rdi,%rdx
  401954:	48 8b 05 85 26 00 00 	mov    0x2685(%rip),%rax        # 403fe0 <stderr@GLIBC_2.2.5>
  40195b:	48 8b 38             	mov    (%rax),%rdi
  40195e:	be 9e 20 40 00       	mov    $0x40209e,%esi
  401963:	31 c0                	xor    %eax,%eax
  401965:	e8 46 f7 ff ff       	call   4010b0 <fprintf@plt>
  40196a:	b8 01 00 00 00       	mov    $0x1,%eax
  40196f:	59                   	pop    %rcx
  401970:	c3                   	ret
  401971:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401978:	0f 1f 84 00 00 00 00 
  40197f:	00 

0000000000401980 <__print_float>:
  401980:	50                   	push   %rax
  401981:	48 8b 05 58 26 00 00 	mov    0x2658(%rip),%rax        # 403fe0 <stderr@GLIBC_2.2.5>
  401988:	48 8b 38             	mov    (%rax),%rdi
  40198b:	c5 fa 5a c0          	vcvtss2sd %xmm0,%xmm0,%xmm0
  40198f:	be a4 20 40 00       	mov    $0x4020a4,%esi
  401994:	b0 01                	mov    $0x1,%al
  401996:	e8 15 f7 ff ff       	call   4010b0 <fprintf@plt>
  40199b:	b8 01 00 00 00       	mov    $0x1,%eax
  4019a0:	59                   	pop    %rcx
  4019a1:	c3                   	ret
  4019a2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4019a9:	1f 84 00 00 00 00 00 

00000000004019b0 <__sqrtf>:
  4019b0:	c5 f0 57 c9          	vxorps %xmm1,%xmm1,%xmm1
  4019b4:	c5 f8 2e c1          	vucomiss %xmm1,%xmm0
  4019b8:	0f 82 22 f7 ff ff    	jb     4010e0 <sqrtf@plt>
  4019be:	c5 fa 51 c0          	vsqrtss %xmm0,%xmm0,%xmm0
  4019c2:	c3                   	ret
  4019c3:	66 66 66 66 2e 0f 1f 	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4019ca:	84 00 00 00 00 00 

00000000004019d0 <__exit>:
  4019d0:	50                   	push   %rax
  4019d1:	31 ff                	xor    %edi,%edi
  4019d3:	e8 48 f7 ff ff       	call   401120 <exit@plt>
  4019d8:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  4019df:	00 

00000000004019e0 <__read_stdin>:
  4019e0:	48 89 f2             	mov    %rsi,%rdx
  4019e3:	48 8b 05 e6 25 00 00 	mov    0x25e6(%rip),%rax        # 403fd0 <stdin@GLIBC_2.2.5>
  4019ea:	48 8b 08             	mov    (%rax),%rcx
  4019ed:	be 01 00 00 00       	mov    $0x1,%esi
  4019f2:	e9 69 f6 ff ff       	jmp    401060 <fread@plt>
  4019f7:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  4019fe:	00 00 

0000000000401a00 <brief_thread_pool_init>:
  401a00:	c3                   	ret
  401a01:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401a08:	0f 1f 84 00 00 00 00 
  401a0f:	00 

0000000000401a10 <brief_barrier_release>:
  401a10:	c3                   	ret
  401a11:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401a18:	0f 1f 84 00 00 00 00 
  401a1f:	00 

0000000000401a20 <brief_barrier_wait>:
  401a20:	c3                   	ret
  401a21:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401a28:	0f 1f 84 00 00 00 00 
  401a2f:	00 

0000000000401a30 <brief_thread_pool_shutdown>:
  401a30:	c3                   	ret

Disassembly of section .fini:

0000000000401a34 <_fini>:
  401a34:	f3 0f 1e fa          	endbr64
  401a38:	48 83 ec 08          	sub    $0x8,%rsp
  401a3c:	48 83 c4 08          	add    $0x8,%rsp
  401a40:	c3                   	ret
