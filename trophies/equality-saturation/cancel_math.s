
trophies/equality-saturation/cancel_math.o:     file format elf64-x86-64


Disassembly of section .text:

0000000000000000 <step>:
   0:	48 89 f8             	mov    %rdi,%rax
   3:	48 8b 4f 08          	mov    0x8(%rdi),%rcx
   7:	48 8b 7f 10          	mov    0x10(%rdi),%rdi
   b:	48 01 cf             	add    %rcx,%rdi
   e:	48 89 78 10          	mov    %rdi,0x10(%rax)
  12:	48 ff c1             	inc    %rcx
  15:	48 89 48 08          	mov    %rcx,0x8(%rax)
  19:	48 b8 a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%rax
  20:	24 03 e5 
  23:	48 0f af c1          	imul   %rcx,%rax
  27:	48 b9 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%rcx
  2e:	6b 00 00 
  31:	48 01 c1             	add    %rax,%rcx
  34:	48 c1 c9 06          	ror    $0x6,%rcx
  38:	48 b8 94 57 53 fe 5a 	movabs $0x35afe535794,%rax
  3f:	03 00 00 
  42:	48 39 c1             	cmp    %rax,%rcx
  45:	0f 86 00 00 00 00    	jbe    4b <step+0x4b>
  4b:	c3                   	ret
  4c:	0f 1f 40 00          	nopl   0x0(%rax)

0000000000000050 <init_state>:
  50:	53                   	push   %rbx
  51:	48 89 fb             	mov    %rdi,%rbx
  54:	48 8d 3d 00 00 00 00 	lea    0x0(%rip),%rdi        # 5b <init_state+0xb>
  5b:	e8 00 00 00 00       	call   60 <init_state+0x10>
  60:	48 89 03             	mov    %rax,(%rbx)
  63:	0f 57 c0             	xorps  %xmm0,%xmm0
  66:	0f 11 43 08          	movups %xmm0,0x8(%rbx)
  6a:	5b                   	pop    %rbx
  6b:	c3                   	ret
  6c:	0f 1f 40 00          	nopl   0x0(%rax)

0000000000000070 <main>:
  70:	55                   	push   %rbp
  71:	41 57                	push   %r15
  73:	41 56                	push   %r14
  75:	41 55                	push   %r13
  77:	41 54                	push   %r12
  79:	53                   	push   %rbx
  7a:	50                   	push   %rax
  7b:	48 8d 3d 00 00 00 00 	lea    0x0(%rip),%rdi        # 82 <main+0x12>
  82:	e8 00 00 00 00       	call   87 <main+0x17>
  87:	48 89 c3             	mov    %rax,%rbx
  8a:	31 ff                	xor    %edi,%edi
  8c:	49 be a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%r14
  93:	24 03 e5 
  96:	49 bf 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%r15
  9d:	6b 00 00 
  a0:	49 bc 94 57 53 fe 5a 	movabs $0x35afe535794,%r12
  a7:	03 00 00 
  aa:	31 c0                	xor    %eax,%eax
  ac:	0f 1f 40 00          	nopl   0x0(%rax)
  b0:	48 39 d8             	cmp    %rbx,%rax
  b3:	7c 1b                	jl     d0 <main+0x60>
  b5:	48 85 ff             	test   %rdi,%rdi
  b8:	78 f6                	js     b0 <main+0x40>
  ba:	48 39 d8             	cmp    %rbx,%rax
  bd:	75 f1                	jne    b0 <main+0x40>
  bf:	eb 39                	jmp    fa <main+0x8a>
  c1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  c8:	0f 1f 84 00 00 00 00 
  cf:	00 
  d0:	48 8d 2c 38          	lea    (%rax,%rdi,1),%rbp
  d4:	4c 8d 68 01          	lea    0x1(%rax),%r13
  d8:	49 0f af c6          	imul   %r14,%rax
  dc:	4c 01 f8             	add    %r15,%rax
  df:	48 c1 c8 06          	ror    $0x6,%rax
  e3:	4c 39 e0             	cmp    %r12,%rax
  e6:	77 05                	ja     ed <main+0x7d>
  e8:	e8 00 00 00 00       	call   ed <main+0x7d>
  ed:	48 89 ef             	mov    %rbp,%rdi
  f0:	4c 89 e8             	mov    %r13,%rax
  f3:	48 85 ff             	test   %rdi,%rdi
  f6:	79 c2                	jns    ba <main+0x4a>
  f8:	eb b6                	jmp    b0 <main+0x40>
  fa:	31 c0                	xor    %eax,%eax
  fc:	48 83 c4 08          	add    $0x8,%rsp
 100:	5b                   	pop    %rbx
 101:	41 5c                	pop    %r12
 103:	41 5d                	pop    %r13
 105:	41 5e                	pop    %r14
 107:	41 5f                	pop    %r15
 109:	5d                   	pop    %rbp
 10a:	c3                   	ret
