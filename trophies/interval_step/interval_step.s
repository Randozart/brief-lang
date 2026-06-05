
benchmarks/interval_step:     file format elf64-x86-64


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
  401158:	48 c7 c7 00 13 40 00 	mov    $0x401300,%rdi
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
  401276:	0f 86 74 06 00 00    	jbe    4018f0 <__print_int>
  40127c:	c3                   	ret
  40127d:	0f 1f 00             	nopl   (%rax)

0000000000401280 <init_state>:
  401280:	53                   	push   %rbx
  401281:	48 89 fb             	mov    %rdi,%rbx
  401284:	bf a0 20 40 00       	mov    $0x4020a0,%edi
  401289:	e8 d2 02 00 00       	call   401560 <__get_env_int>
  40128e:	48 89 03             	mov    %rax,(%rbx)
  401291:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  401295:	c5 f8 11 43 08       	vmovups %xmm0,0x8(%rbx)
  40129a:	5b                   	pop    %rbx
  40129b:	c3                   	ret
  40129c:	0f 1f 40 00          	nopl   0x0(%rax)

00000000004012a0 <reactor_tick>:
  4012a0:	48 8b 47 08          	mov    0x8(%rdi),%rax
  4012a4:	48 3b 07             	cmp    (%rdi),%rax
  4012a7:	7d 48                	jge    4012f1 <reactor_tick+0x51>
  4012a9:	48 89 f9             	mov    %rdi,%rcx
  4012ac:	48 8b 7f 10          	mov    0x10(%rdi),%rdi
  4012b0:	48 01 c7             	add    %rax,%rdi
  4012b3:	48 89 79 10          	mov    %rdi,0x10(%rcx)
  4012b7:	48 ff c0             	inc    %rax
  4012ba:	48 89 41 08          	mov    %rax,0x8(%rcx)
  4012be:	48 b9 a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%rcx
  4012c5:	24 03 e5 
  4012c8:	48 0f af c8          	imul   %rax,%rcx
  4012cc:	48 b8 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%rax
  4012d3:	6b 00 00 
  4012d6:	48 01 c8             	add    %rcx,%rax
  4012d9:	48 0f ac c0 06       	shrd   $0x6,%rax,%rax
  4012de:	48 b9 94 57 53 fe 5a 	movabs $0x35afe535794,%rcx
  4012e5:	03 00 00 
  4012e8:	48 39 c8             	cmp    %rcx,%rax
  4012eb:	0f 86 ff 05 00 00    	jbe    4018f0 <__print_int>
  4012f1:	c3                   	ret
  4012f2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4012f9:	1f 84 00 00 00 00 00 

0000000000401300 <main>:
  401300:	55                   	push   %rbp
  401301:	41 57                	push   %r15
  401303:	41 56                	push   %r14
  401305:	41 55                	push   %r13
  401307:	41 54                	push   %r12
  401309:	53                   	push   %rbx
  40130a:	50                   	push   %rax
  40130b:	bf a0 20 40 00       	mov    $0x4020a0,%edi
  401310:	e8 4b 02 00 00       	call   401560 <__get_env_int>
  401315:	48 89 c3             	mov    %rax,%rbx
  401318:	45 31 f6             	xor    %r14d,%r14d
  40131b:	49 bf a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%r15
  401322:	24 03 e5 
  401325:	49 bc 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%r12
  40132c:	6b 00 00 
  40132f:	49 bd 94 57 53 fe 5a 	movabs $0x35afe535794,%r13
  401336:	03 00 00 
  401339:	31 ed                	xor    %ebp,%ebp
  40133b:	0f 1f 44 00 00       	nopl   0x0(%rax,%rax,1)
  401340:	48 39 dd             	cmp    %rbx,%rbp
  401343:	7d 1a                	jge    40135f <main+0x5f>
  401345:	49 01 ee             	add    %rbp,%r14
  401348:	48 ff c5             	inc    %rbp
  40134b:	48 89 e8             	mov    %rbp,%rax
  40134e:	49 0f af c7          	imul   %r15,%rax
  401352:	4c 01 e0             	add    %r12,%rax
  401355:	48 0f ac c0 06       	shrd   $0x6,%rax,%rax
  40135a:	4c 39 e8             	cmp    %r13,%rax
  40135d:	76 0c                	jbe    40136b <main+0x6b>
  40135f:	4d 85 f6             	test   %r14,%r14
  401362:	78 dc                	js     401340 <main+0x40>
  401364:	48 39 dd             	cmp    %rbx,%rbp
  401367:	75 d7                	jne    401340 <main+0x40>
  401369:	eb 0f                	jmp    40137a <main+0x7a>
  40136b:	4c 89 f7             	mov    %r14,%rdi
  40136e:	e8 7d 05 00 00       	call   4018f0 <__print_int>
  401373:	4d 85 f6             	test   %r14,%r14
  401376:	79 ec                	jns    401364 <main+0x64>
  401378:	eb c6                	jmp    401340 <main+0x40>
  40137a:	31 c0                	xor    %eax,%eax
  40137c:	48 83 c4 08          	add    $0x8,%rsp
  401380:	5b                   	pop    %rbx
  401381:	41 5c                	pop    %r12
  401383:	41 5d                	pop    %r13
  401385:	41 5e                	pop    %r14
  401387:	41 5f                	pop    %r15
  401389:	5d                   	pop    %rbp
  40138a:	c3                   	ret
  40138b:	0f 1f 44 00 00       	nopl   0x0(%rax,%rax,1)

0000000000401390 <brief_rt_ctor>:
  401390:	e9 0b 00 00 00       	jmp    4013a0 <__rt_init>
  401395:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  40139c:	00 00 00 00 

00000000004013a0 <__rt_init>:
  4013a0:	53                   	push   %rbx
  4013a1:	48 81 ec 00 01 00 00 	sub    $0x100,%rsp
  4013a8:	be 20 15 40 00       	mov    $0x401520,%esi
  4013ad:	bf 02 00 00 00       	mov    $0x2,%edi
  4013b2:	e8 e9 fc ff ff       	call   4010a0 <signal@plt>
  4013b7:	be 30 15 40 00       	mov    $0x401530,%esi
  4013bc:	bf 0f 00 00 00       	mov    $0xf,%edi
  4013c1:	e8 da fc ff ff       	call   4010a0 <signal@plt>
  4013c6:	be 40 15 40 00       	mov    $0x401540,%esi
  4013cb:	bf 01 00 00 00       	mov    $0x1,%edi
  4013d0:	e8 cb fc ff ff       	call   4010a0 <signal@plt>
  4013d5:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4013d9:	c5 fc 11 84 24 e0 00 	vmovups %ymm0,0xe0(%rsp)
  4013e0:	00 00 
  4013e2:	c5 fc 11 84 24 d0 00 	vmovups %ymm0,0xd0(%rsp)
  4013e9:	00 00 
  4013eb:	c5 fc 11 84 24 b0 00 	vmovups %ymm0,0xb0(%rsp)
  4013f2:	00 00 
  4013f4:	c5 fc 11 84 24 90 00 	vmovups %ymm0,0x90(%rsp)
  4013fb:	00 00 
  4013fd:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  401403:	48 c7 44 24 68 50 15 	movq   $0x401550,0x68(%rsp)
  40140a:	40 00 
  40140c:	c7 84 24 f0 00 00 00 	movl   $0x4,0xf0(%rsp)
  401413:	04 00 00 00 
  401417:	c5 f8 77             	vzeroupper
  40141a:	e8 f1 fc ff ff       	call   401110 <__libc_current_sigrtmin@plt>
  40141f:	8d 78 01             	lea    0x1(%rax),%edi
  401422:	48 8d 5c 24 68       	lea    0x68(%rsp),%rbx
  401427:	48 89 de             	mov    %rbx,%rsi
  40142a:	31 d2                	xor    %edx,%edx
  40142c:	e8 1f fc ff ff       	call   401050 <sigaction@plt>
  401431:	e8 da fc ff ff       	call   401110 <__libc_current_sigrtmin@plt>
  401436:	8d 78 02             	lea    0x2(%rax),%edi
  401439:	48 89 de             	mov    %rbx,%rsi
  40143c:	31 d2                	xor    %edx,%edx
  40143e:	e8 0d fc ff ff       	call   401050 <sigaction@plt>
  401443:	e8 c8 fc ff ff       	call   401110 <__libc_current_sigrtmin@plt>
  401448:	ff c0                	inc    %eax
  40144a:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  40144e:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
  401453:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
  401459:	89 44 24 08          	mov    %eax,0x8(%rsp)
  40145d:	48 89 e6             	mov    %rsp,%rsi
  401460:	ba c8 40 40 00       	mov    $0x4040c8,%edx
  401465:	31 ff                	xor    %edi,%edi
  401467:	c5 f8 77             	vzeroupper
  40146a:	e8 01 fc ff ff       	call   401070 <timer_create@plt>
  40146f:	85 c0                	test   %eax,%eax
  401471:	75 27                	jne    40149a <__rt_init+0xfa>
  401473:	c4 e2 7d 1a 05 e4 0b 	vbroadcastf128 0xbe4(%rip),%ymm0        # 402060 <_IO_stdin_used+0x60>
  40147a:	00 00 
  40147c:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
  401482:	48 8b 3d 3f 2c 00 00 	mov    0x2c3f(%rip),%rdi        # 4040c8 <g_timer_1hz>
  401489:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
  40148e:	31 f6                	xor    %esi,%esi
  401490:	31 c9                	xor    %ecx,%ecx
  401492:	c5 f8 77             	vzeroupper
  401495:	e8 a6 fb ff ff       	call   401040 <timer_settime@plt>
  40149a:	e8 71 fc ff ff       	call   401110 <__libc_current_sigrtmin@plt>
  40149f:	83 c0 02             	add    $0x2,%eax
  4014a2:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4014a6:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
  4014ab:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
  4014b1:	89 44 24 08          	mov    %eax,0x8(%rsp)
  4014b5:	48 89 e6             	mov    %rsp,%rsi
  4014b8:	ba d0 40 40 00       	mov    $0x4040d0,%edx
  4014bd:	31 ff                	xor    %edi,%edi
  4014bf:	c5 f8 77             	vzeroupper
  4014c2:	e8 a9 fb ff ff       	call   401070 <timer_create@plt>
  4014c7:	85 c0                	test   %eax,%eax
  4014c9:	75 27                	jne    4014f2 <__rt_init+0x152>
  4014cb:	c4 e2 7d 1a 05 9c 0b 	vbroadcastf128 0xb9c(%rip),%ymm0        # 402070 <_IO_stdin_used+0x70>
  4014d2:	00 00 
  4014d4:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
  4014da:	48 8b 3d ef 2b 00 00 	mov    0x2bef(%rip),%rdi        # 4040d0 <g_timer_100hz>
  4014e1:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
  4014e6:	31 f6                	xor    %esi,%esi
  4014e8:	31 c9                	xor    %ecx,%ecx
  4014ea:	c5 f8 77             	vzeroupper
  4014ed:	e8 4e fb ff ff       	call   401040 <timer_settime@plt>
  4014f2:	48 8b 05 cf 2a 00 00 	mov    0x2acf(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  4014f9:	48 8b 38             	mov    (%rax),%rdi
  4014fc:	31 f6                	xor    %esi,%esi
  4014fe:	ba 01 00 00 00       	mov    $0x1,%edx
  401503:	31 c9                	xor    %ecx,%ecx
  401505:	e8 f6 fb ff ff       	call   401100 <setvbuf@plt>
  40150a:	c6 05 90 2b 00 00 01 	movb   $0x1,0x2b90(%rip)        # 4040a1 <__io_pending>
  401511:	48 81 c4 00 01 00 00 	add    $0x100,%rsp
  401518:	5b                   	pop    %rbx
  401519:	c3                   	ret
  40151a:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

0000000000401520 <handle_sigint>:
  401520:	c6 05 7b 2b 00 00 01 	movb   $0x1,0x2b7b(%rip)        # 4040a2 <__sigint_flag>
  401527:	c6 05 73 2b 00 00 01 	movb   $0x1,0x2b73(%rip)        # 4040a1 <__io_pending>
  40152e:	c3                   	ret
  40152f:	90                   	nop

0000000000401530 <handle_sigterm>:
  401530:	c6 05 6c 2b 00 00 01 	movb   $0x1,0x2b6c(%rip)        # 4040a3 <__sigterm_flag>
  401537:	c6 05 63 2b 00 00 01 	movb   $0x1,0x2b63(%rip)        # 4040a1 <__io_pending>
  40153e:	c3                   	ret
  40153f:	90                   	nop

0000000000401540 <handle_sighup>:
  401540:	c6 05 5d 2b 00 00 01 	movb   $0x1,0x2b5d(%rip)        # 4040a4 <__sighup_flag>
  401547:	c6 05 53 2b 00 00 01 	movb   $0x1,0x2b53(%rip)        # 4040a1 <__io_pending>
  40154e:	c3                   	ret
  40154f:	90                   	nop

0000000000401550 <handle_timer>:
  401550:	48 ff 05 51 2b 00 00 	incq   0x2b51(%rip)        # 4040a8 <__timer_1hz>
  401557:	c6 05 43 2b 00 00 01 	movb   $0x1,0x2b43(%rip)        # 4040a1 <__io_pending>
  40155e:	c3                   	ret
  40155f:	90                   	nop

0000000000401560 <__get_env_int>:
  401560:	53                   	push   %rbx
  401561:	48 83 ec 10          	sub    $0x10,%rsp
  401565:	e8 c6 fa ff ff       	call   401030 <getenv@plt>
  40156a:	48 85 c0             	test   %rax,%rax
  40156d:	74 32                	je     4015a1 <__get_env_int+0x41>
  40156f:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
  401576:	00 00 
  401578:	48 8d 74 24 08       	lea    0x8(%rsp),%rsi
  40157d:	48 89 c7             	mov    %rax,%rdi
  401580:	ba 0a 00 00 00       	mov    $0xa,%edx
  401585:	48 89 c3             	mov    %rax,%rbx
  401588:	e8 33 fb ff ff       	call   4010c0 <strtol@plt>
  40158d:	48 89 c1             	mov    %rax,%rcx
  401590:	31 c0                	xor    %eax,%eax
  401592:	48 39 5c 24 08       	cmp    %rbx,0x8(%rsp)
  401597:	48 0f 45 c1          	cmovne %rcx,%rax
  40159b:	48 83 c4 10          	add    $0x10,%rsp
  40159f:	5b                   	pop    %rbx
  4015a0:	c3                   	ret
  4015a1:	31 c0                	xor    %eax,%eax
  4015a3:	48 83 c4 10          	add    $0x10,%rsp
  4015a7:	5b                   	pop    %rbx
  4015a8:	c3                   	ret
  4015a9:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

00000000004015b0 <__rt_wait>:
  4015b0:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
  4015b7:	8b 3d db 2a 00 00    	mov    0x2adb(%rip),%edi        # 404098 <g_epoll_fd>
  4015bd:	85 ff                	test   %edi,%edi
  4015bf:	79 3f                	jns    401600 <__rt_wait+0x50>
  4015c1:	31 ff                	xor    %edi,%edi
  4015c3:	e8 68 fb ff ff       	call   401130 <epoll_create1@plt>
  4015c8:	89 05 ca 2a 00 00    	mov    %eax,0x2aca(%rip)        # 404098 <g_epoll_fd>
  4015ce:	85 c0                	test   %eax,%eax
  4015d0:	0f 88 d5 00 00 00    	js     4016ab <__rt_wait+0xfb>
  4015d6:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
  4015dd:	00 
  4015de:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
  4015e5:	00 00 
  4015e7:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
  4015ec:	89 c7                	mov    %eax,%edi
  4015ee:	be 01 00 00 00       	mov    $0x1,%esi
  4015f3:	31 d2                	xor    %edx,%edx
  4015f5:	e8 96 fa ff ff       	call   401090 <epoll_ctl@plt>
  4015fa:	8b 3d 98 2a 00 00    	mov    0x2a98(%rip),%edi        # 404098 <g_epoll_fd>
  401600:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  401605:	ba 40 00 00 00       	mov    $0x40,%edx
  40160a:	b9 64 00 00 00       	mov    $0x64,%ecx
  40160f:	e8 dc fa ff ff       	call   4010f0 <epoll_wait@plt>
  401614:	85 c0                	test   %eax,%eax
  401616:	0f 8e ef 00 00 00    	jle    40170b <__rt_wait+0x15b>
  40161c:	89 c1                	mov    %eax,%ecx
  40161e:	83 f8 01             	cmp    $0x1,%eax
  401621:	75 1e                	jne    401641 <__rt_wait+0x91>
  401623:	31 c0                	xor    %eax,%eax
  401625:	f6 c1 01             	test   $0x1,%cl
  401628:	74 0f                	je     401639 <__rt_wait+0x89>
  40162a:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
  40162e:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
  401633:	0f 84 e1 00 00 00    	je     40171a <__rt_wait+0x16a>
  401639:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  401640:	c3                   	ret
  401641:	89 c8                	mov    %ecx,%eax
  401643:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
  401648:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
  40164d:	48 89 c6             	mov    %rax,%rsi
  401650:	eb 18                	jmp    40166a <__rt_wait+0xba>
  401652:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401659:	1f 84 00 00 00 00 00 
  401660:	48 83 c2 18          	add    $0x18,%rdx
  401664:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
  401668:	74 bb                	je     401625 <__rt_wait+0x75>
  40166a:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
  40166e:	75 20                	jne    401690 <__rt_wait+0xe0>
  401670:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
  401674:	74 1a                	je     401690 <__rt_wait+0xe0>
  401676:	c6 05 23 2a 00 00 01 	movb   $0x1,0x2a23(%rip)        # 4040a0 <__stdin_ready>
  40167d:	c6 05 1d 2a 00 00 01 	movb   $0x1,0x2a1d(%rip)        # 4040a1 <__io_pending>
  401684:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
  40168b:	00 00 00 00 00 
  401690:	83 3a 00             	cmpl   $0x0,(%rdx)
  401693:	75 cb                	jne    401660 <__rt_wait+0xb0>
  401695:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
  401699:	74 c5                	je     401660 <__rt_wait+0xb0>
  40169b:	c6 05 fe 29 00 00 01 	movb   $0x1,0x29fe(%rip)        # 4040a0 <__stdin_ready>
  4016a2:	c6 05 f8 29 00 00 01 	movb   $0x1,0x29f8(%rip)        # 4040a1 <__io_pending>
  4016a9:	eb b5                	jmp    401660 <__rt_wait+0xb0>
  4016ab:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4016af:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  4016b5:	c5 fc 11 44 24 58    	vmovups %ymm0,0x58(%rsp)
  4016bb:	c5 fc 11 44 24 38    	vmovups %ymm0,0x38(%rsp)
  4016c1:	c5 fc 11 44 24 18    	vmovups %ymm0,0x18(%rsp)
  4016c7:	48 c7 44 24 10 01 00 	movq   $0x1,0x10(%rsp)
  4016ce:	00 00 
  4016d0:	c5 f8 10 05 a8 09 00 	vmovups 0x9a8(%rip),%xmm0        # 402080 <_IO_stdin_used+0x80>
  4016d7:	00 
  4016d8:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
  4016dd:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  4016e2:	49 89 e0             	mov    %rsp,%r8
  4016e5:	bf 01 00 00 00       	mov    $0x1,%edi
  4016ea:	31 d2                	xor    %edx,%edx
  4016ec:	31 c9                	xor    %ecx,%ecx
  4016ee:	c5 f8 77             	vzeroupper
  4016f1:	e8 da f9 ff ff       	call   4010d0 <select@plt>
  4016f6:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
  4016fb:	74 0e                	je     40170b <__rt_wait+0x15b>
  4016fd:	c6 05 9c 29 00 00 01 	movb   $0x1,0x299c(%rip)        # 4040a0 <__stdin_ready>
  401704:	c6 05 96 29 00 00 01 	movb   $0x1,0x2996(%rip)        # 4040a1 <__io_pending>
  40170b:	c6 05 8f 29 00 00 01 	movb   $0x1,0x298f(%rip)        # 4040a1 <__io_pending>
  401712:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  401719:	c3                   	ret
  40171a:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
  40171f:	0f 84 14 ff ff ff    	je     401639 <__rt_wait+0x89>
  401725:	c6 05 74 29 00 00 01 	movb   $0x1,0x2974(%rip)        # 4040a0 <__stdin_ready>
  40172c:	c6 05 6e 29 00 00 01 	movb   $0x1,0x296e(%rip)        # 4040a1 <__io_pending>
  401733:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  40173a:	c3                   	ret
  40173b:	0f 1f 44 00 00       	nopl   0x0(%rax,%rax,1)

0000000000401740 <__rt_poll>:
  401740:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
  401747:	8b 3d 4b 29 00 00    	mov    0x294b(%rip),%edi        # 404098 <g_epoll_fd>
  40174d:	85 ff                	test   %edi,%edi
  40174f:	79 3f                	jns    401790 <__rt_poll+0x50>
  401751:	31 ff                	xor    %edi,%edi
  401753:	e8 d8 f9 ff ff       	call   401130 <epoll_create1@plt>
  401758:	89 05 3a 29 00 00    	mov    %eax,0x293a(%rip)        # 404098 <g_epoll_fd>
  40175e:	85 c0                	test   %eax,%eax
  401760:	0f 88 d5 00 00 00    	js     40183b <__rt_poll+0xfb>
  401766:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
  40176d:	00 
  40176e:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
  401775:	00 00 
  401777:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
  40177c:	89 c7                	mov    %eax,%edi
  40177e:	be 01 00 00 00       	mov    $0x1,%esi
  401783:	31 d2                	xor    %edx,%edx
  401785:	e8 06 f9 ff ff       	call   401090 <epoll_ctl@plt>
  40178a:	8b 3d 08 29 00 00    	mov    0x2908(%rip),%edi        # 404098 <g_epoll_fd>
  401790:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  401795:	ba 40 00 00 00       	mov    $0x40,%edx
  40179a:	31 c9                	xor    %ecx,%ecx
  40179c:	e8 4f f9 ff ff       	call   4010f0 <epoll_wait@plt>
  4017a1:	85 c0                	test   %eax,%eax
  4017a3:	7e 1d                	jle    4017c2 <__rt_poll+0x82>
  4017a5:	89 c1                	mov    %eax,%ecx
  4017a7:	83 f8 01             	cmp    $0x1,%eax
  4017aa:	75 25                	jne    4017d1 <__rt_poll+0x91>
  4017ac:	31 c0                	xor    %eax,%eax
  4017ae:	f6 c1 01             	test   $0x1,%cl
  4017b1:	74 0f                	je     4017c2 <__rt_poll+0x82>
  4017b3:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
  4017b7:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
  4017bc:	0f 84 cd 00 00 00    	je     40188f <__rt_poll+0x14f>
  4017c2:	c6 05 d8 28 00 00 01 	movb   $0x1,0x28d8(%rip)        # 4040a1 <__io_pending>
  4017c9:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  4017d0:	c3                   	ret
  4017d1:	89 c8                	mov    %ecx,%eax
  4017d3:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
  4017d8:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
  4017dd:	48 89 c6             	mov    %rax,%rsi
  4017e0:	eb 18                	jmp    4017fa <__rt_poll+0xba>
  4017e2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4017e9:	1f 84 00 00 00 00 00 
  4017f0:	48 83 c2 18          	add    $0x18,%rdx
  4017f4:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
  4017f8:	74 b4                	je     4017ae <__rt_poll+0x6e>
  4017fa:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
  4017fe:	75 20                	jne    401820 <__rt_poll+0xe0>
  401800:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
  401804:	74 1a                	je     401820 <__rt_poll+0xe0>
  401806:	c6 05 93 28 00 00 01 	movb   $0x1,0x2893(%rip)        # 4040a0 <__stdin_ready>
  40180d:	c6 05 8d 28 00 00 01 	movb   $0x1,0x288d(%rip)        # 4040a1 <__io_pending>
  401814:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
  40181b:	00 00 00 00 00 
  401820:	83 3a 00             	cmpl   $0x0,(%rdx)
  401823:	75 cb                	jne    4017f0 <__rt_poll+0xb0>
  401825:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
  401829:	74 c5                	je     4017f0 <__rt_poll+0xb0>
  40182b:	c6 05 6e 28 00 00 01 	movb   $0x1,0x286e(%rip)        # 4040a0 <__stdin_ready>
  401832:	c6 05 68 28 00 00 01 	movb   $0x1,0x2868(%rip)        # 4040a1 <__io_pending>
  401839:	eb b5                	jmp    4017f0 <__rt_poll+0xb0>
  40183b:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  40183f:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  401845:	c5 fc 11 44 24 58    	vmovups %ymm0,0x58(%rsp)
  40184b:	c5 fc 11 44 24 38    	vmovups %ymm0,0x38(%rsp)
  401851:	c5 fc 11 44 24 18    	vmovups %ymm0,0x18(%rsp)
  401857:	48 c7 44 24 10 01 00 	movq   $0x1,0x10(%rsp)
  40185e:	00 00 
  401860:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  401864:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
  401869:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  40186e:	49 89 e0             	mov    %rsp,%r8
  401871:	bf 01 00 00 00       	mov    $0x1,%edi
  401876:	31 d2                	xor    %edx,%edx
  401878:	31 c9                	xor    %ecx,%ecx
  40187a:	c5 f8 77             	vzeroupper
  40187d:	e8 4e f8 ff ff       	call   4010d0 <select@plt>
  401882:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
  401887:	0f 84 35 ff ff ff    	je     4017c2 <__rt_poll+0x82>
  40188d:	eb 0b                	jmp    40189a <__rt_poll+0x15a>
  40188f:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
  401894:	0f 84 28 ff ff ff    	je     4017c2 <__rt_poll+0x82>
  40189a:	c6 05 ff 27 00 00 01 	movb   $0x1,0x27ff(%rip)        # 4040a0 <__stdin_ready>
  4018a1:	c6 05 f9 27 00 00 01 	movb   $0x1,0x27f9(%rip)        # 4040a1 <__io_pending>
  4018a8:	c6 05 f2 27 00 00 01 	movb   $0x1,0x27f2(%rip)        # 4040a1 <__io_pending>
  4018af:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  4018b6:	c3                   	ret
  4018b7:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  4018be:	00 00 

00000000004018c0 <__wait_for_event>:
  4018c0:	e9 eb fc ff ff       	jmp    4015b0 <__rt_wait>
  4018c5:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  4018cc:	00 00 00 00 

00000000004018d0 <__print>:
  4018d0:	50                   	push   %rax
  4018d1:	48 8b 05 f0 26 00 00 	mov    0x26f0(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  4018d8:	48 8b 30             	mov    (%rax),%rsi
  4018db:	e8 a0 f7 ff ff       	call   401080 <fputs@plt>
  4018e0:	b8 01 00 00 00       	mov    $0x1,%eax
  4018e5:	59                   	pop    %rcx
  4018e6:	c3                   	ret
  4018e7:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  4018ee:	00 00 

00000000004018f0 <__print_int>:
  4018f0:	50                   	push   %rax
  4018f1:	48 89 fa             	mov    %rdi,%rdx
  4018f4:	48 8b 05 e5 26 00 00 	mov    0x26e5(%rip),%rax        # 403fe0 <stderr@GLIBC_2.2.5>
  4018fb:	48 8b 38             	mov    (%rax),%rdi
  4018fe:	be a6 20 40 00       	mov    $0x4020a6,%esi
  401903:	31 c0                	xor    %eax,%eax
  401905:	e8 a6 f7 ff ff       	call   4010b0 <fprintf@plt>
  40190a:	b8 01 00 00 00       	mov    $0x1,%eax
  40190f:	59                   	pop    %rcx
  401910:	c3                   	ret
  401911:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401918:	0f 1f 84 00 00 00 00 
  40191f:	00 

0000000000401920 <__print_float>:
  401920:	50                   	push   %rax
  401921:	48 8b 05 b8 26 00 00 	mov    0x26b8(%rip),%rax        # 403fe0 <stderr@GLIBC_2.2.5>
  401928:	48 8b 38             	mov    (%rax),%rdi
  40192b:	c5 fa 5a c0          	vcvtss2sd %xmm0,%xmm0,%xmm0
  40192f:	be ac 20 40 00       	mov    $0x4020ac,%esi
  401934:	b0 01                	mov    $0x1,%al
  401936:	e8 75 f7 ff ff       	call   4010b0 <fprintf@plt>
  40193b:	b8 01 00 00 00       	mov    $0x1,%eax
  401940:	59                   	pop    %rcx
  401941:	c3                   	ret
  401942:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401949:	1f 84 00 00 00 00 00 

0000000000401950 <__sqrtf>:
  401950:	c5 f0 57 c9          	vxorps %xmm1,%xmm1,%xmm1
  401954:	c5 f8 2e c1          	vucomiss %xmm1,%xmm0
  401958:	0f 82 82 f7 ff ff    	jb     4010e0 <sqrtf@plt>
  40195e:	c5 fa 51 c0          	vsqrtss %xmm0,%xmm0,%xmm0
  401962:	c3                   	ret
  401963:	66 66 66 66 2e 0f 1f 	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  40196a:	84 00 00 00 00 00 

0000000000401970 <__exit>:
  401970:	50                   	push   %rax
  401971:	31 ff                	xor    %edi,%edi
  401973:	e8 a8 f7 ff ff       	call   401120 <exit@plt>
  401978:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  40197f:	00 

0000000000401980 <__read_stdin>:
  401980:	48 89 f2             	mov    %rsi,%rdx
  401983:	48 8b 05 46 26 00 00 	mov    0x2646(%rip),%rax        # 403fd0 <stdin@GLIBC_2.2.5>
  40198a:	48 8b 08             	mov    (%rax),%rcx
  40198d:	be 01 00 00 00       	mov    $0x1,%esi
  401992:	e9 c9 f6 ff ff       	jmp    401060 <fread@plt>
  401997:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  40199e:	00 00 

00000000004019a0 <brief_thread_pool_init>:
  4019a0:	c3                   	ret
  4019a1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4019a8:	0f 1f 84 00 00 00 00 
  4019af:	00 

00000000004019b0 <brief_barrier_release>:
  4019b0:	c3                   	ret
  4019b1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4019b8:	0f 1f 84 00 00 00 00 
  4019bf:	00 

00000000004019c0 <brief_barrier_wait>:
  4019c0:	c3                   	ret
  4019c1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4019c8:	0f 1f 84 00 00 00 00 
  4019cf:	00 

00000000004019d0 <brief_thread_pool_shutdown>:
  4019d0:	c3                   	ret

Disassembly of section .fini:

00000000004019d4 <_fini>:
  4019d4:	f3 0f 1e fa          	endbr64
  4019d8:	48 83 ec 08          	sub    $0x8,%rsp
  4019dc:	48 83 c4 08          	add    $0x8,%rsp
  4019e0:	c3                   	ret
