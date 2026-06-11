
trophies/intrinsics/example.o:     file format elf64-x86-64


Disassembly of section .text:

0000000000000000 <compute>:
   0:	41 56                	push   %r14
   2:	53                   	push   %rbx
   3:	50                   	push   %rax
   4:	48 89 fb             	mov    %rdi,%rbx
   7:	4c 8b 77 08          	mov    0x8(%rdi),%r14
   b:	f3 0f 10 05 00 00 00 	movss  0x0(%rip),%xmm0        # 13 <compute+0x13>
  12:	00 
  13:	e8 00 00 00 00       	call   18 <compute+0x18>
  18:	49 ff c6             	inc    %r14
  1b:	4c 89 73 08          	mov    %r14,0x8(%rbx)
  1f:	48 83 c4 08          	add    $0x8,%rsp
  23:	5b                   	pop    %rbx
  24:	41 5e                	pop    %r14
  26:	c3                   	ret
  27:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  2e:	00 00 

0000000000000030 <__init>:
  30:	f3 0f 10 05 00 00 00 	movss  0x0(%rip),%xmm0        # 38 <__init+0x8>
  37:	00 
  38:	e9 00 00 00 00       	jmp    3d <__init+0xd>
  3d:	0f 1f 00             	nopl   (%rax)

0000000000000040 <async_body_compute>:
  40:	41 56                	push   %r14
  42:	53                   	push   %rbx
  43:	50                   	push   %rax
  44:	4c 8b 77 08          	mov    0x8(%rdi),%r14
  48:	4c 3b 77 10          	cmp    0x10(%rdi),%r14
  4c:	7d 17                	jge    65 <async_body_compute+0x25>
  4e:	48 89 fb             	mov    %rdi,%rbx
  51:	f3 0f 10 05 00 00 00 	movss  0x0(%rip),%xmm0        # 59 <async_body_compute+0x19>
  58:	00 
  59:	e8 00 00 00 00       	call   5e <async_body_compute+0x1e>
  5e:	49 ff c6             	inc    %r14
  61:	4c 89 73 08          	mov    %r14,0x8(%rbx)
  65:	48 83 c4 08          	add    $0x8,%rsp
  69:	5b                   	pop    %rbx
  6a:	41 5e                	pop    %r14
  6c:	c3                   	ret
  6d:	0f 1f 00             	nopl   (%rax)

0000000000000070 <async_body___init>:
  70:	80 3f 01             	cmpb   $0x1,(%rdi)
  73:	75 01                	jne    76 <async_body___init+0x6>
  75:	c3                   	ret
  76:	f3 0f 10 05 00 00 00 	movss  0x0(%rip),%xmm0        # 7e <async_body___init+0xe>
  7d:	00 
  7e:	e9 00 00 00 00       	jmp    83 <async_body___init+0x13>
  83:	66 66 66 66 2e 0f 1f 	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  8a:	84 00 00 00 00 00 

0000000000000090 <init_state>:
  90:	53                   	push   %rbx
  91:	48 89 fb             	mov    %rdi,%rbx
  94:	c6 07 00             	movb   $0x0,(%rdi)
  97:	48 c7 47 08 00 00 00 	movq   $0x0,0x8(%rdi)
  9e:	00 
  9f:	48 8d 3d 00 00 00 00 	lea    0x0(%rip),%rdi        # a6 <init_state+0x16>
  a6:	e8 00 00 00 00       	call   ab <init_state+0x1b>
  ab:	48 89 43 10          	mov    %rax,0x10(%rbx)
  af:	5b                   	pop    %rbx
  b0:	c3                   	ret
  b1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  b8:	0f 1f 84 00 00 00 00 
  bf:	00 

00000000000000c0 <reactor_tick>:
  c0:	55                   	push   %rbp
  c1:	41 56                	push   %r14
  c3:	53                   	push   %rbx
  c4:	4c 8b 77 08          	mov    0x8(%rdi),%r14
  c8:	0f b6 2f             	movzbl (%rdi),%ebp
  cb:	4c 3b 77 10          	cmp    0x10(%rdi),%r14
  cf:	7d 17                	jge    e8 <reactor_tick+0x28>
  d1:	48 89 fb             	mov    %rdi,%rbx
  d4:	f3 0f 10 05 00 00 00 	movss  0x0(%rip),%xmm0        # dc <reactor_tick+0x1c>
  db:	00 
  dc:	e8 00 00 00 00       	call   e1 <reactor_tick+0x21>
  e1:	49 ff c6             	inc    %r14
  e4:	4c 89 73 08          	mov    %r14,0x8(%rbx)
  e8:	40 80 fd 01          	cmp    $0x1,%bpl
  ec:	75 05                	jne    f3 <reactor_tick+0x33>
  ee:	5b                   	pop    %rbx
  ef:	41 5e                	pop    %r14
  f1:	5d                   	pop    %rbp
  f2:	c3                   	ret
  f3:	f3 0f 10 05 00 00 00 	movss  0x0(%rip),%xmm0        # fb <reactor_tick+0x3b>
  fa:	00 
  fb:	5b                   	pop    %rbx
  fc:	41 5e                	pop    %r14
  fe:	5d                   	pop    %rbp
  ff:	e9 00 00 00 00       	jmp    104 <reactor_tick+0x44>
 104:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
 10b:	00 00 00 00 00 

0000000000000110 <main>:
 110:	41 56                	push   %r14
 112:	53                   	push   %rbx
 113:	50                   	push   %rax
 114:	48 8d 3d 00 00 00 00 	lea    0x0(%rip),%rdi        # 11b <main+0xb>
 11b:	e8 00 00 00 00       	call   120 <main+0x10>
 120:	48 89 c3             	mov    %rax,%rbx
 123:	48 8d 35 00 00 00 00 	lea    0x0(%rip),%rsi        # 12a <main+0x1a>
 12a:	bf 02 00 00 00       	mov    $0x2,%edi
 12f:	e8 00 00 00 00       	call   134 <main+0x24>
 134:	45 31 f6             	xor    %r14d,%r14d
 137:	eb 1e                	jmp    157 <main+0x47>
 139:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)
 140:	f3 0f 10 05 00 00 00 	movss  0x0(%rip),%xmm0        # 148 <main+0x38>
 147:	00 
 148:	e8 00 00 00 00       	call   14d <main+0x3d>
 14d:	e8 00 00 00 00       	call   152 <main+0x42>
 152:	49 39 de             	cmp    %rbx,%r14
 155:	74 1c                	je     173 <main+0x63>
 157:	e8 00 00 00 00       	call   15c <main+0x4c>
 15c:	49 39 de             	cmp    %rbx,%r14
 15f:	7d df                	jge    140 <main+0x30>
 161:	f3 0f 10 05 00 00 00 	movss  0x0(%rip),%xmm0        # 169 <main+0x59>
 168:	00 
 169:	e8 00 00 00 00       	call   16e <main+0x5e>
 16e:	49 ff c6             	inc    %r14
 171:	eb cd                	jmp    140 <main+0x30>
 173:	31 c0                	xor    %eax,%eax
 175:	48 83 c4 08          	add    $0x8,%rsp
 179:	5b                   	pop    %rbx
 17a:	41 5e                	pop    %r14
 17c:	c3                   	ret
