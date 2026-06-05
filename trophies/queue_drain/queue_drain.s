
benchmarks/queue_drain:     file format elf64-x86-64


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
  401158:	48 c7 c7 c0 12 40 00 	mov    $0x4012c0,%rdi
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

0000000000401230 <work>:
  401230:	48 89 f8             	mov    %rdi,%rax
  401233:	48 8b 7f 08          	mov    0x8(%rdi),%rdi
  401237:	48 8b 48 10          	mov    0x10(%rax),%rcx
  40123b:	48 8b 51 08          	mov    0x8(%rcx),%rdx
  40123f:	48 89 7c d1 08       	mov    %rdi,0x8(%rcx,%rdx,8)
  401244:	48 89 51 08          	mov    %rdx,0x8(%rcx)
  401248:	48 ff c7             	inc    %rdi
  40124b:	48 89 78 08          	mov    %rdi,0x8(%rax)
  40124f:	48 b8 a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%rax
  401256:	24 03 e5 
  401259:	48 0f af c7          	imul   %rdi,%rax
  40125d:	48 b9 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%rcx
  401264:	6b 00 00 
  401267:	48 01 c1             	add    %rax,%rcx
  40126a:	48 0f ac c9 06       	shrd   $0x6,%rcx,%rcx
  40126f:	48 b8 94 57 53 fe 5a 	movabs $0x35afe535794,%rax
  401276:	03 00 00 
  401279:	48 39 c1             	cmp    %rax,%rcx
  40127c:	0f 86 fe 06 00 00    	jbe    401980 <__print_int>
  401282:	c3                   	ret
  401283:	66 66 66 66 2e 0f 1f 	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  40128a:	84 00 00 00 00 00 

0000000000401290 <init_state>:
  401290:	53                   	push   %rbx
  401291:	48 83 ec 20          	sub    $0x20,%rsp
  401295:	48 89 fb             	mov    %rdi,%rbx
  401298:	bf 90 20 40 00       	mov    $0x402090,%edi
  40129d:	e8 4e 03 00 00       	call   4015f0 <__get_env_int>
  4012a2:	48 89 03             	mov    %rax,(%rbx)
  4012a5:	48 c7 43 08 00 00 00 	movq   $0x0,0x8(%rbx)
  4012ac:	00 
  4012ad:	48 8d 44 24 08       	lea    0x8(%rsp),%rax
  4012b2:	48 89 43 10          	mov    %rax,0x10(%rbx)
  4012b6:	48 83 c4 20          	add    $0x20,%rsp
  4012ba:	5b                   	pop    %rbx
  4012bb:	c3                   	ret
  4012bc:	0f 1f 40 00          	nopl   0x0(%rax)

00000000004012c0 <main>:
  4012c0:	55                   	push   %rbp
  4012c1:	41 57                	push   %r15
  4012c3:	41 56                	push   %r14
  4012c5:	41 55                	push   %r13
  4012c7:	41 54                	push   %r12
  4012c9:	53                   	push   %rbx
  4012ca:	48 83 ec 28          	sub    $0x28,%rsp
  4012ce:	bf 90 20 40 00       	mov    $0x402090,%edi
  4012d3:	e8 18 03 00 00       	call   4015f0 <__get_env_int>
  4012d8:	48 89 44 24 08       	mov    %rax,0x8(%rsp)
  4012dd:	4c 8d 78 fd          	lea    -0x3(%rax),%r15
  4012e1:	31 db                	xor    %ebx,%ebx
  4012e3:	49 bc a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%r12
  4012ea:	24 03 e5 
  4012ed:	49 bd 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%r13
  4012f4:	6b 00 00 
  4012f7:	48 bd 94 57 53 fe 5a 	movabs $0x35afe535794,%rbp
  4012fe:	03 00 00 
  401301:	eb 15                	jmp    401318 <main+0x58>
  401303:	66 66 66 66 2e 0f 1f 	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  40130a:	84 00 00 00 00 00 
  401310:	4c 89 f7             	mov    %r14,%rdi
  401313:	e8 68 06 00 00       	call   401980 <__print_int>
  401318:	49 89 de             	mov    %rbx,%r14
  40131b:	4c 39 fb             	cmp    %r15,%rbx
  40131e:	0f 8d 9c 00 00 00    	jge    4013c0 <main+0x100>
  401324:	48 8b 5c 24 18       	mov    0x18(%rsp),%rbx
  401329:	4c 89 f0             	mov    %r14,%rax
  40132c:	49 0f af c4          	imul   %r12,%rax
  401330:	4c 01 e8             	add    %r13,%rax
  401333:	48 0f ac c0 06       	shrd   $0x6,%rax,%rax
  401338:	48 39 e8             	cmp    %rbp,%rax
  40133b:	77 08                	ja     401345 <main+0x85>
  40133d:	4c 89 f7             	mov    %r14,%rdi
  401340:	e8 3b 06 00 00       	call   401980 <__print_int>
  401345:	49 8d 7e 01          	lea    0x1(%r14),%rdi
  401349:	48 89 f8             	mov    %rdi,%rax
  40134c:	49 0f af c4          	imul   %r12,%rax
  401350:	4c 01 e8             	add    %r13,%rax
  401353:	48 0f ac c0 06       	shrd   $0x6,%rax,%rax
  401358:	48 39 e8             	cmp    %rbp,%rax
  40135b:	77 05                	ja     401362 <main+0xa2>
  40135d:	e8 1e 06 00 00       	call   401980 <__print_int>
  401362:	49 8d 7e 02          	lea    0x2(%r14),%rdi
  401366:	48 89 f8             	mov    %rdi,%rax
  401369:	49 0f af c4          	imul   %r12,%rax
  40136d:	4c 01 e8             	add    %r13,%rax
  401370:	48 0f ac c0 06       	shrd   $0x6,%rax,%rax
  401375:	48 39 e8             	cmp    %rbp,%rax
  401378:	77 05                	ja     40137f <main+0xbf>
  40137a:	e8 01 06 00 00       	call   401980 <__print_int>
  40137f:	49 8d 46 03          	lea    0x3(%r14),%rax
  401383:	48 89 44 dc 18       	mov    %rax,0x18(%rsp,%rbx,8)
  401388:	48 89 5c 24 18       	mov    %rbx,0x18(%rsp)
  40138d:	4c 89 f3             	mov    %r14,%rbx
  401390:	48 83 c3 04          	add    $0x4,%rbx
  401394:	49 89 c6             	mov    %rax,%r14
  401397:	49 0f af c4          	imul   %r12,%rax
  40139b:	4c 01 e8             	add    %r13,%rax
  40139e:	48 0f ac c0 06       	shrd   $0x6,%rax,%rax
  4013a3:	48 39 e8             	cmp    %rbp,%rax
  4013a6:	0f 87 6c ff ff ff    	ja     401318 <main+0x58>
  4013ac:	e9 5f ff ff ff       	jmp    401310 <main+0x50>
  4013b1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4013b8:	0f 1f 84 00 00 00 00 
  4013bf:	00 
  4013c0:	4c 3b 74 24 08       	cmp    0x8(%rsp),%r14
  4013c5:	7d 3a                	jge    401401 <main+0x141>
  4013c7:	48 8b 44 24 18       	mov    0x18(%rsp),%rax
  4013cc:	4c 89 74 c4 18       	mov    %r14,0x18(%rsp,%rax,8)
  4013d1:	48 89 44 24 18       	mov    %rax,0x18(%rsp)
  4013d6:	49 8d 5e 01          	lea    0x1(%r14),%rbx
  4013da:	4c 89 f0             	mov    %r14,%rax
  4013dd:	49 0f af c4          	imul   %r12,%rax
  4013e1:	4c 01 e8             	add    %r13,%rax
  4013e4:	48 0f ac c0 06       	shrd   $0x6,%rax,%rax
  4013e9:	48 b9 95 57 53 fe 5a 	movabs $0x35afe535795,%rcx
  4013f0:	03 00 00 
  4013f3:	48 39 c8             	cmp    %rcx,%rax
  4013f6:	0f 83 1c ff ff ff    	jae    401318 <main+0x58>
  4013fc:	e9 0f ff ff ff       	jmp    401310 <main+0x50>
  401401:	31 c0                	xor    %eax,%eax
  401403:	48 83 c4 28          	add    $0x28,%rsp
  401407:	5b                   	pop    %rbx
  401408:	41 5c                	pop    %r12
  40140a:	41 5d                	pop    %r13
  40140c:	41 5e                	pop    %r14
  40140e:	41 5f                	pop    %r15
  401410:	5d                   	pop    %rbp
  401411:	c3                   	ret
  401412:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401419:	1f 84 00 00 00 00 00 

0000000000401420 <brief_rt_ctor>:
  401420:	e9 0b 00 00 00       	jmp    401430 <__rt_init>
  401425:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  40142c:	00 00 00 00 

0000000000401430 <__rt_init>:
  401430:	53                   	push   %rbx
  401431:	48 81 ec 00 01 00 00 	sub    $0x100,%rsp
  401438:	be b0 15 40 00       	mov    $0x4015b0,%esi
  40143d:	bf 02 00 00 00       	mov    $0x2,%edi
  401442:	e8 59 fc ff ff       	call   4010a0 <signal@plt>
  401447:	be c0 15 40 00       	mov    $0x4015c0,%esi
  40144c:	bf 0f 00 00 00       	mov    $0xf,%edi
  401451:	e8 4a fc ff ff       	call   4010a0 <signal@plt>
  401456:	be d0 15 40 00       	mov    $0x4015d0,%esi
  40145b:	bf 01 00 00 00       	mov    $0x1,%edi
  401460:	e8 3b fc ff ff       	call   4010a0 <signal@plt>
  401465:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  401469:	c5 fc 11 84 24 e0 00 	vmovups %ymm0,0xe0(%rsp)
  401470:	00 00 
  401472:	c5 fc 11 84 24 d0 00 	vmovups %ymm0,0xd0(%rsp)
  401479:	00 00 
  40147b:	c5 fc 11 84 24 b0 00 	vmovups %ymm0,0xb0(%rsp)
  401482:	00 00 
  401484:	c5 fc 11 84 24 90 00 	vmovups %ymm0,0x90(%rsp)
  40148b:	00 00 
  40148d:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  401493:	48 c7 44 24 68 e0 15 	movq   $0x4015e0,0x68(%rsp)
  40149a:	40 00 
  40149c:	c7 84 24 f0 00 00 00 	movl   $0x4,0xf0(%rsp)
  4014a3:	04 00 00 00 
  4014a7:	c5 f8 77             	vzeroupper
  4014aa:	e8 61 fc ff ff       	call   401110 <__libc_current_sigrtmin@plt>
  4014af:	8d 78 01             	lea    0x1(%rax),%edi
  4014b2:	48 8d 5c 24 68       	lea    0x68(%rsp),%rbx
  4014b7:	48 89 de             	mov    %rbx,%rsi
  4014ba:	31 d2                	xor    %edx,%edx
  4014bc:	e8 8f fb ff ff       	call   401050 <sigaction@plt>
  4014c1:	e8 4a fc ff ff       	call   401110 <__libc_current_sigrtmin@plt>
  4014c6:	8d 78 02             	lea    0x2(%rax),%edi
  4014c9:	48 89 de             	mov    %rbx,%rsi
  4014cc:	31 d2                	xor    %edx,%edx
  4014ce:	e8 7d fb ff ff       	call   401050 <sigaction@plt>
  4014d3:	e8 38 fc ff ff       	call   401110 <__libc_current_sigrtmin@plt>
  4014d8:	ff c0                	inc    %eax
  4014da:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4014de:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
  4014e3:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
  4014e9:	89 44 24 08          	mov    %eax,0x8(%rsp)
  4014ed:	48 89 e6             	mov    %rsp,%rsi
  4014f0:	ba c8 40 40 00       	mov    $0x4040c8,%edx
  4014f5:	31 ff                	xor    %edi,%edi
  4014f7:	c5 f8 77             	vzeroupper
  4014fa:	e8 71 fb ff ff       	call   401070 <timer_create@plt>
  4014ff:	85 c0                	test   %eax,%eax
  401501:	75 27                	jne    40152a <__rt_init+0xfa>
  401503:	c4 e2 7d 1a 05 54 0b 	vbroadcastf128 0xb54(%rip),%ymm0        # 402060 <_IO_stdin_used+0x60>
  40150a:	00 00 
  40150c:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
  401512:	48 8b 3d af 2b 00 00 	mov    0x2baf(%rip),%rdi        # 4040c8 <g_timer_1hz>
  401519:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
  40151e:	31 f6                	xor    %esi,%esi
  401520:	31 c9                	xor    %ecx,%ecx
  401522:	c5 f8 77             	vzeroupper
  401525:	e8 16 fb ff ff       	call   401040 <timer_settime@plt>
  40152a:	e8 e1 fb ff ff       	call   401110 <__libc_current_sigrtmin@plt>
  40152f:	83 c0 02             	add    $0x2,%eax
  401532:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  401536:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
  40153b:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
  401541:	89 44 24 08          	mov    %eax,0x8(%rsp)
  401545:	48 89 e6             	mov    %rsp,%rsi
  401548:	ba d0 40 40 00       	mov    $0x4040d0,%edx
  40154d:	31 ff                	xor    %edi,%edi
  40154f:	c5 f8 77             	vzeroupper
  401552:	e8 19 fb ff ff       	call   401070 <timer_create@plt>
  401557:	85 c0                	test   %eax,%eax
  401559:	75 27                	jne    401582 <__rt_init+0x152>
  40155b:	c4 e2 7d 1a 05 0c 0b 	vbroadcastf128 0xb0c(%rip),%ymm0        # 402070 <_IO_stdin_used+0x70>
  401562:	00 00 
  401564:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
  40156a:	48 8b 3d 5f 2b 00 00 	mov    0x2b5f(%rip),%rdi        # 4040d0 <g_timer_100hz>
  401571:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
  401576:	31 f6                	xor    %esi,%esi
  401578:	31 c9                	xor    %ecx,%ecx
  40157a:	c5 f8 77             	vzeroupper
  40157d:	e8 be fa ff ff       	call   401040 <timer_settime@plt>
  401582:	48 8b 05 3f 2a 00 00 	mov    0x2a3f(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401589:	48 8b 38             	mov    (%rax),%rdi
  40158c:	31 f6                	xor    %esi,%esi
  40158e:	ba 01 00 00 00       	mov    $0x1,%edx
  401593:	31 c9                	xor    %ecx,%ecx
  401595:	e8 66 fb ff ff       	call   401100 <setvbuf@plt>
  40159a:	c6 05 00 2b 00 00 01 	movb   $0x1,0x2b00(%rip)        # 4040a1 <__io_pending>
  4015a1:	48 81 c4 00 01 00 00 	add    $0x100,%rsp
  4015a8:	5b                   	pop    %rbx
  4015a9:	c3                   	ret
  4015aa:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

00000000004015b0 <handle_sigint>:
  4015b0:	c6 05 eb 2a 00 00 01 	movb   $0x1,0x2aeb(%rip)        # 4040a2 <__sigint_flag>
  4015b7:	c6 05 e3 2a 00 00 01 	movb   $0x1,0x2ae3(%rip)        # 4040a1 <__io_pending>
  4015be:	c3                   	ret
  4015bf:	90                   	nop

00000000004015c0 <handle_sigterm>:
  4015c0:	c6 05 dc 2a 00 00 01 	movb   $0x1,0x2adc(%rip)        # 4040a3 <__sigterm_flag>
  4015c7:	c6 05 d3 2a 00 00 01 	movb   $0x1,0x2ad3(%rip)        # 4040a1 <__io_pending>
  4015ce:	c3                   	ret
  4015cf:	90                   	nop

00000000004015d0 <handle_sighup>:
  4015d0:	c6 05 cd 2a 00 00 01 	movb   $0x1,0x2acd(%rip)        # 4040a4 <__sighup_flag>
  4015d7:	c6 05 c3 2a 00 00 01 	movb   $0x1,0x2ac3(%rip)        # 4040a1 <__io_pending>
  4015de:	c3                   	ret
  4015df:	90                   	nop

00000000004015e0 <handle_timer>:
  4015e0:	48 ff 05 c1 2a 00 00 	incq   0x2ac1(%rip)        # 4040a8 <__timer_1hz>
  4015e7:	c6 05 b3 2a 00 00 01 	movb   $0x1,0x2ab3(%rip)        # 4040a1 <__io_pending>
  4015ee:	c3                   	ret
  4015ef:	90                   	nop

00000000004015f0 <__get_env_int>:
  4015f0:	53                   	push   %rbx
  4015f1:	48 83 ec 10          	sub    $0x10,%rsp
  4015f5:	e8 36 fa ff ff       	call   401030 <getenv@plt>
  4015fa:	48 85 c0             	test   %rax,%rax
  4015fd:	74 32                	je     401631 <__get_env_int+0x41>
  4015ff:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
  401606:	00 00 
  401608:	48 8d 74 24 08       	lea    0x8(%rsp),%rsi
  40160d:	48 89 c7             	mov    %rax,%rdi
  401610:	ba 0a 00 00 00       	mov    $0xa,%edx
  401615:	48 89 c3             	mov    %rax,%rbx
  401618:	e8 a3 fa ff ff       	call   4010c0 <strtol@plt>
  40161d:	48 89 c1             	mov    %rax,%rcx
  401620:	31 c0                	xor    %eax,%eax
  401622:	48 39 5c 24 08       	cmp    %rbx,0x8(%rsp)
  401627:	48 0f 45 c1          	cmovne %rcx,%rax
  40162b:	48 83 c4 10          	add    $0x10,%rsp
  40162f:	5b                   	pop    %rbx
  401630:	c3                   	ret
  401631:	31 c0                	xor    %eax,%eax
  401633:	48 83 c4 10          	add    $0x10,%rsp
  401637:	5b                   	pop    %rbx
  401638:	c3                   	ret
  401639:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000401640 <__rt_wait>:
  401640:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
  401647:	8b 3d 4b 2a 00 00    	mov    0x2a4b(%rip),%edi        # 404098 <g_epoll_fd>
  40164d:	85 ff                	test   %edi,%edi
  40164f:	79 3f                	jns    401690 <__rt_wait+0x50>
  401651:	31 ff                	xor    %edi,%edi
  401653:	e8 d8 fa ff ff       	call   401130 <epoll_create1@plt>
  401658:	89 05 3a 2a 00 00    	mov    %eax,0x2a3a(%rip)        # 404098 <g_epoll_fd>
  40165e:	85 c0                	test   %eax,%eax
  401660:	0f 88 d5 00 00 00    	js     40173b <__rt_wait+0xfb>
  401666:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
  40166d:	00 
  40166e:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
  401675:	00 00 
  401677:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
  40167c:	89 c7                	mov    %eax,%edi
  40167e:	be 01 00 00 00       	mov    $0x1,%esi
  401683:	31 d2                	xor    %edx,%edx
  401685:	e8 06 fa ff ff       	call   401090 <epoll_ctl@plt>
  40168a:	8b 3d 08 2a 00 00    	mov    0x2a08(%rip),%edi        # 404098 <g_epoll_fd>
  401690:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  401695:	ba 40 00 00 00       	mov    $0x40,%edx
  40169a:	b9 64 00 00 00       	mov    $0x64,%ecx
  40169f:	e8 4c fa ff ff       	call   4010f0 <epoll_wait@plt>
  4016a4:	85 c0                	test   %eax,%eax
  4016a6:	0f 8e ef 00 00 00    	jle    40179b <__rt_wait+0x15b>
  4016ac:	89 c1                	mov    %eax,%ecx
  4016ae:	83 f8 01             	cmp    $0x1,%eax
  4016b1:	75 1e                	jne    4016d1 <__rt_wait+0x91>
  4016b3:	31 c0                	xor    %eax,%eax
  4016b5:	f6 c1 01             	test   $0x1,%cl
  4016b8:	74 0f                	je     4016c9 <__rt_wait+0x89>
  4016ba:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
  4016be:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
  4016c3:	0f 84 e1 00 00 00    	je     4017aa <__rt_wait+0x16a>
  4016c9:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  4016d0:	c3                   	ret
  4016d1:	89 c8                	mov    %ecx,%eax
  4016d3:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
  4016d8:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
  4016dd:	48 89 c6             	mov    %rax,%rsi
  4016e0:	eb 18                	jmp    4016fa <__rt_wait+0xba>
  4016e2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4016e9:	1f 84 00 00 00 00 00 
  4016f0:	48 83 c2 18          	add    $0x18,%rdx
  4016f4:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
  4016f8:	74 bb                	je     4016b5 <__rt_wait+0x75>
  4016fa:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
  4016fe:	75 20                	jne    401720 <__rt_wait+0xe0>
  401700:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
  401704:	74 1a                	je     401720 <__rt_wait+0xe0>
  401706:	c6 05 93 29 00 00 01 	movb   $0x1,0x2993(%rip)        # 4040a0 <__stdin_ready>
  40170d:	c6 05 8d 29 00 00 01 	movb   $0x1,0x298d(%rip)        # 4040a1 <__io_pending>
  401714:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
  40171b:	00 00 00 00 00 
  401720:	83 3a 00             	cmpl   $0x0,(%rdx)
  401723:	75 cb                	jne    4016f0 <__rt_wait+0xb0>
  401725:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
  401729:	74 c5                	je     4016f0 <__rt_wait+0xb0>
  40172b:	c6 05 6e 29 00 00 01 	movb   $0x1,0x296e(%rip)        # 4040a0 <__stdin_ready>
  401732:	c6 05 68 29 00 00 01 	movb   $0x1,0x2968(%rip)        # 4040a1 <__io_pending>
  401739:	eb b5                	jmp    4016f0 <__rt_wait+0xb0>
  40173b:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  40173f:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  401745:	c5 fc 11 44 24 58    	vmovups %ymm0,0x58(%rsp)
  40174b:	c5 fc 11 44 24 38    	vmovups %ymm0,0x38(%rsp)
  401751:	c5 fc 11 44 24 18    	vmovups %ymm0,0x18(%rsp)
  401757:	48 c7 44 24 10 01 00 	movq   $0x1,0x10(%rsp)
  40175e:	00 00 
  401760:	c5 f8 10 05 18 09 00 	vmovups 0x918(%rip),%xmm0        # 402080 <_IO_stdin_used+0x80>
  401767:	00 
  401768:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
  40176d:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  401772:	49 89 e0             	mov    %rsp,%r8
  401775:	bf 01 00 00 00       	mov    $0x1,%edi
  40177a:	31 d2                	xor    %edx,%edx
  40177c:	31 c9                	xor    %ecx,%ecx
  40177e:	c5 f8 77             	vzeroupper
  401781:	e8 4a f9 ff ff       	call   4010d0 <select@plt>
  401786:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
  40178b:	74 0e                	je     40179b <__rt_wait+0x15b>
  40178d:	c6 05 0c 29 00 00 01 	movb   $0x1,0x290c(%rip)        # 4040a0 <__stdin_ready>
  401794:	c6 05 06 29 00 00 01 	movb   $0x1,0x2906(%rip)        # 4040a1 <__io_pending>
  40179b:	c6 05 ff 28 00 00 01 	movb   $0x1,0x28ff(%rip)        # 4040a1 <__io_pending>
  4017a2:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  4017a9:	c3                   	ret
  4017aa:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
  4017af:	0f 84 14 ff ff ff    	je     4016c9 <__rt_wait+0x89>
  4017b5:	c6 05 e4 28 00 00 01 	movb   $0x1,0x28e4(%rip)        # 4040a0 <__stdin_ready>
  4017bc:	c6 05 de 28 00 00 01 	movb   $0x1,0x28de(%rip)        # 4040a1 <__io_pending>
  4017c3:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  4017ca:	c3                   	ret
  4017cb:	0f 1f 44 00 00       	nopl   0x0(%rax,%rax,1)

00000000004017d0 <__rt_poll>:
  4017d0:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
  4017d7:	8b 3d bb 28 00 00    	mov    0x28bb(%rip),%edi        # 404098 <g_epoll_fd>
  4017dd:	85 ff                	test   %edi,%edi
  4017df:	79 3f                	jns    401820 <__rt_poll+0x50>
  4017e1:	31 ff                	xor    %edi,%edi
  4017e3:	e8 48 f9 ff ff       	call   401130 <epoll_create1@plt>
  4017e8:	89 05 aa 28 00 00    	mov    %eax,0x28aa(%rip)        # 404098 <g_epoll_fd>
  4017ee:	85 c0                	test   %eax,%eax
  4017f0:	0f 88 d5 00 00 00    	js     4018cb <__rt_poll+0xfb>
  4017f6:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
  4017fd:	00 
  4017fe:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
  401805:	00 00 
  401807:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
  40180c:	89 c7                	mov    %eax,%edi
  40180e:	be 01 00 00 00       	mov    $0x1,%esi
  401813:	31 d2                	xor    %edx,%edx
  401815:	e8 76 f8 ff ff       	call   401090 <epoll_ctl@plt>
  40181a:	8b 3d 78 28 00 00    	mov    0x2878(%rip),%edi        # 404098 <g_epoll_fd>
  401820:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  401825:	ba 40 00 00 00       	mov    $0x40,%edx
  40182a:	31 c9                	xor    %ecx,%ecx
  40182c:	e8 bf f8 ff ff       	call   4010f0 <epoll_wait@plt>
  401831:	85 c0                	test   %eax,%eax
  401833:	7e 1d                	jle    401852 <__rt_poll+0x82>
  401835:	89 c1                	mov    %eax,%ecx
  401837:	83 f8 01             	cmp    $0x1,%eax
  40183a:	75 25                	jne    401861 <__rt_poll+0x91>
  40183c:	31 c0                	xor    %eax,%eax
  40183e:	f6 c1 01             	test   $0x1,%cl
  401841:	74 0f                	je     401852 <__rt_poll+0x82>
  401843:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
  401847:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
  40184c:	0f 84 cd 00 00 00    	je     40191f <__rt_poll+0x14f>
  401852:	c6 05 48 28 00 00 01 	movb   $0x1,0x2848(%rip)        # 4040a1 <__io_pending>
  401859:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  401860:	c3                   	ret
  401861:	89 c8                	mov    %ecx,%eax
  401863:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
  401868:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
  40186d:	48 89 c6             	mov    %rax,%rsi
  401870:	eb 18                	jmp    40188a <__rt_poll+0xba>
  401872:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401879:	1f 84 00 00 00 00 00 
  401880:	48 83 c2 18          	add    $0x18,%rdx
  401884:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
  401888:	74 b4                	je     40183e <__rt_poll+0x6e>
  40188a:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
  40188e:	75 20                	jne    4018b0 <__rt_poll+0xe0>
  401890:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
  401894:	74 1a                	je     4018b0 <__rt_poll+0xe0>
  401896:	c6 05 03 28 00 00 01 	movb   $0x1,0x2803(%rip)        # 4040a0 <__stdin_ready>
  40189d:	c6 05 fd 27 00 00 01 	movb   $0x1,0x27fd(%rip)        # 4040a1 <__io_pending>
  4018a4:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
  4018ab:	00 00 00 00 00 
  4018b0:	83 3a 00             	cmpl   $0x0,(%rdx)
  4018b3:	75 cb                	jne    401880 <__rt_poll+0xb0>
  4018b5:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
  4018b9:	74 c5                	je     401880 <__rt_poll+0xb0>
  4018bb:	c6 05 de 27 00 00 01 	movb   $0x1,0x27de(%rip)        # 4040a0 <__stdin_ready>
  4018c2:	c6 05 d8 27 00 00 01 	movb   $0x1,0x27d8(%rip)        # 4040a1 <__io_pending>
  4018c9:	eb b5                	jmp    401880 <__rt_poll+0xb0>
  4018cb:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4018cf:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
  4018d5:	c5 fc 11 44 24 58    	vmovups %ymm0,0x58(%rsp)
  4018db:	c5 fc 11 44 24 38    	vmovups %ymm0,0x38(%rsp)
  4018e1:	c5 fc 11 44 24 18    	vmovups %ymm0,0x18(%rsp)
  4018e7:	48 c7 44 24 10 01 00 	movq   $0x1,0x10(%rsp)
  4018ee:	00 00 
  4018f0:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
  4018f4:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
  4018f9:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
  4018fe:	49 89 e0             	mov    %rsp,%r8
  401901:	bf 01 00 00 00       	mov    $0x1,%edi
  401906:	31 d2                	xor    %edx,%edx
  401908:	31 c9                	xor    %ecx,%ecx
  40190a:	c5 f8 77             	vzeroupper
  40190d:	e8 be f7 ff ff       	call   4010d0 <select@plt>
  401912:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
  401917:	0f 84 35 ff ff ff    	je     401852 <__rt_poll+0x82>
  40191d:	eb 0b                	jmp    40192a <__rt_poll+0x15a>
  40191f:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
  401924:	0f 84 28 ff ff ff    	je     401852 <__rt_poll+0x82>
  40192a:	c6 05 6f 27 00 00 01 	movb   $0x1,0x276f(%rip)        # 4040a0 <__stdin_ready>
  401931:	c6 05 69 27 00 00 01 	movb   $0x1,0x2769(%rip)        # 4040a1 <__io_pending>
  401938:	c6 05 62 27 00 00 01 	movb   $0x1,0x2762(%rip)        # 4040a1 <__io_pending>
  40193f:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
  401946:	c3                   	ret
  401947:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  40194e:	00 00 

0000000000401950 <__wait_for_event>:
  401950:	e9 eb fc ff ff       	jmp    401640 <__rt_wait>
  401955:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
  40195c:	00 00 00 00 

0000000000401960 <__print>:
  401960:	50                   	push   %rax
  401961:	48 8b 05 60 26 00 00 	mov    0x2660(%rip),%rax        # 403fc8 <stdout@GLIBC_2.2.5>
  401968:	48 8b 30             	mov    (%rax),%rsi
  40196b:	e8 10 f7 ff ff       	call   401080 <fputs@plt>
  401970:	b8 01 00 00 00       	mov    $0x1,%eax
  401975:	59                   	pop    %rcx
  401976:	c3                   	ret
  401977:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  40197e:	00 00 

0000000000401980 <__print_int>:
  401980:	50                   	push   %rax
  401981:	48 89 fa             	mov    %rdi,%rdx
  401984:	48 8b 05 55 26 00 00 	mov    0x2655(%rip),%rax        # 403fe0 <stderr@GLIBC_2.2.5>
  40198b:	48 8b 38             	mov    (%rax),%rdi
  40198e:	be 96 20 40 00       	mov    $0x402096,%esi
  401993:	31 c0                	xor    %eax,%eax
  401995:	e8 16 f7 ff ff       	call   4010b0 <fprintf@plt>
  40199a:	b8 01 00 00 00       	mov    $0x1,%eax
  40199f:	59                   	pop    %rcx
  4019a0:	c3                   	ret
  4019a1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4019a8:	0f 1f 84 00 00 00 00 
  4019af:	00 

00000000004019b0 <__print_float>:
  4019b0:	50                   	push   %rax
  4019b1:	48 8b 05 28 26 00 00 	mov    0x2628(%rip),%rax        # 403fe0 <stderr@GLIBC_2.2.5>
  4019b8:	48 8b 38             	mov    (%rax),%rdi
  4019bb:	c5 fa 5a c0          	vcvtss2sd %xmm0,%xmm0,%xmm0
  4019bf:	be 9c 20 40 00       	mov    $0x40209c,%esi
  4019c4:	b0 01                	mov    $0x1,%al
  4019c6:	e8 e5 f6 ff ff       	call   4010b0 <fprintf@plt>
  4019cb:	b8 01 00 00 00       	mov    $0x1,%eax
  4019d0:	59                   	pop    %rcx
  4019d1:	c3                   	ret
  4019d2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4019d9:	1f 84 00 00 00 00 00 

00000000004019e0 <__sqrtf>:
  4019e0:	c5 f0 57 c9          	vxorps %xmm1,%xmm1,%xmm1
  4019e4:	c5 f8 2e c1          	vucomiss %xmm1,%xmm0
  4019e8:	0f 82 f2 f6 ff ff    	jb     4010e0 <sqrtf@plt>
  4019ee:	c5 fa 51 c0          	vsqrtss %xmm0,%xmm0,%xmm0
  4019f2:	c3                   	ret
  4019f3:	66 66 66 66 2e 0f 1f 	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  4019fa:	84 00 00 00 00 00 

0000000000401a00 <__exit>:
  401a00:	50                   	push   %rax
  401a01:	31 ff                	xor    %edi,%edi
  401a03:	e8 18 f7 ff ff       	call   401120 <exit@plt>
  401a08:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  401a0f:	00 

0000000000401a10 <__read_stdin>:
  401a10:	48 89 f2             	mov    %rsi,%rdx
  401a13:	48 8b 05 b6 25 00 00 	mov    0x25b6(%rip),%rax        # 403fd0 <stdin@GLIBC_2.2.5>
  401a1a:	48 8b 08             	mov    (%rax),%rcx
  401a1d:	be 01 00 00 00       	mov    $0x1,%esi
  401a22:	e9 39 f6 ff ff       	jmp    401060 <fread@plt>
  401a27:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  401a2e:	00 00 

0000000000401a30 <brief_thread_pool_init>:
  401a30:	c3                   	ret
  401a31:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401a38:	0f 1f 84 00 00 00 00 
  401a3f:	00 

0000000000401a40 <brief_barrier_release>:
  401a40:	c3                   	ret
  401a41:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401a48:	0f 1f 84 00 00 00 00 
  401a4f:	00 

0000000000401a50 <brief_barrier_wait>:
  401a50:	c3                   	ret
  401a51:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  401a58:	0f 1f 84 00 00 00 00 
  401a5f:	00 

0000000000401a60 <brief_thread_pool_shutdown>:
  401a60:	c3                   	ret

Disassembly of section .fini:

0000000000401a64 <_fini>:
  401a64:	f3 0f 1e fa          	endbr64
  401a68:	48 83 ec 08          	sub    $0x8,%rsp
  401a6c:	48 83 c4 08          	add    $0x8,%rsp
  401a70:	c3                   	ret
