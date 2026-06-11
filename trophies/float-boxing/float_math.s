
trophies/float-boxing/float_math.o:     file format elf64-x86-64


Disassembly of section .text:

0000000000000000 <tick>:
   0:	48 8b 47 30          	mov    0x30(%rdi),%rax
   4:	f2 0f 10 47 04       	movsd  0x4(%rdi),%xmm0
   9:	0f 28 15 00 00 00 00 	movaps 0x0(%rip),%xmm2        # 10 <tick+0x10>
  10:	0f 59 d0             	mulps  %xmm0,%xmm2
  13:	f3 0f 10 0f          	movss  (%rdi),%xmm1
  17:	0f 14 c8             	unpcklps %xmm0,%xmm1
  1a:	0f 58 ca             	addps  %xmm2,%xmm1
  1d:	0f 13 0f             	movlps %xmm1,(%rdi)
  20:	f3 0f 10 1d 00 00 00 	movss  0x0(%rip),%xmm3        # 28 <tick+0x28>
  27:	00 
  28:	f3 0f 10 57 0c       	movss  0xc(%rdi),%xmm2
  2d:	f3 0f 58 d3          	addss  %xmm3,%xmm2
  31:	f3 0f 11 57 0c       	movss  %xmm2,0xc(%rdi)
  36:	f3 0f 10 67 1c       	movss  0x1c(%rdi),%xmm4
  3b:	f3 0f 58 e3          	addss  %xmm3,%xmm4
  3f:	f3 0f 11 67 1c       	movss  %xmm4,0x1c(%rdi)
  44:	f3 0f 58 5f 2c       	addss  0x2c(%rdi),%xmm3
  49:	f3 0f 11 5f 2c       	movss  %xmm3,0x2c(%rdi)
  4e:	48 ff c0             	inc    %rax
  51:	48 89 47 30          	mov    %rax,0x30(%rdi)
  55:	48 b9 a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%rcx
  5c:	24 03 e5 
  5f:	48 0f af c8          	imul   %rax,%rcx
  63:	48 b8 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%rax
  6a:	6b 00 00 
  6d:	48 01 c8             	add    %rcx,%rax
  70:	48 c1 c8 06          	ror    $0x6,%rax
  74:	48 b9 94 57 53 fe 5a 	movabs $0x35afe535794,%rcx
  7b:	03 00 00 
  7e:	48 39 c8             	cmp    %rcx,%rax
  81:	76 01                	jbe    84 <tick+0x84>
  83:	c3                   	ret
  84:	f2 0f 10 6f 14       	movsd  0x14(%rdi),%xmm5
  89:	f2 0f 10 77 20       	movsd  0x20(%rdi),%xmm6
  8e:	0f 58 f5             	addps  %xmm5,%xmm6
  91:	0f c6 c0 55          	shufps $0x55,%xmm0,%xmm0
  95:	0f 58 c1             	addps  %xmm1,%xmm0
  98:	0f 28 ee             	movaps %xmm6,%xmm5
  9b:	0f c6 ee 55          	shufps $0x55,%xmm6,%xmm5
  9f:	f3 0f 58 ee          	addss  %xmm6,%xmm5
  a3:	0f 14 eb             	unpcklps %xmm3,%xmm5
  a6:	f3 0f 10 5f 10       	movss  0x10(%rdi),%xmm3
  ab:	0f 14 dc             	unpcklps %xmm4,%xmm3
  ae:	0f 58 dd             	addps  %xmm5,%xmm3
  b1:	f3 0f 10 67 28       	movss  0x28(%rdi),%xmm4
  b6:	0f 14 e2             	unpcklps %xmm2,%xmm4
  b9:	0f c6 c8 01          	shufps $0x1,%xmm0,%xmm1
  bd:	0f c6 c8 e2          	shufps $0xe2,%xmm0,%xmm1
  c1:	0f 58 cc             	addps  %xmm4,%xmm1
  c4:	0f 58 cb             	addps  %xmm3,%xmm1
  c7:	0f 28 c1             	movaps %xmm1,%xmm0
  ca:	0f c6 c1 55          	shufps $0x55,%xmm1,%xmm0
  ce:	f3 0f 58 c1          	addss  %xmm1,%xmm0
  d2:	e9 00 00 00 00       	jmp    d7 <tick+0xd7>
  d7:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  de:	00 00 

00000000000000e0 <init_state>:
  e0:	53                   	push   %rbx
  e1:	48 89 fb             	mov    %rdi,%rbx
  e4:	0f 57 c0             	xorps  %xmm0,%xmm0
  e7:	0f 11 47 20          	movups %xmm0,0x20(%rdi)
  eb:	0f 11 47 10          	movups %xmm0,0x10(%rdi)
  ef:	0f 11 07             	movups %xmm0,(%rdi)
  f2:	48 c7 47 30 00 00 00 	movq   $0x0,0x30(%rdi)
  f9:	00 
  fa:	48 8d 3d 00 00 00 00 	lea    0x0(%rip),%rdi        # 101 <init_state+0x21>
 101:	e8 00 00 00 00       	call   106 <init_state+0x26>
 106:	48 89 43 38          	mov    %rax,0x38(%rbx)
 10a:	5b                   	pop    %rbx
 10b:	c3                   	ret
 10c:	0f 1f 40 00          	nopl   0x0(%rax)

0000000000000110 <main>:
 110:	41 57                	push   %r15
 112:	41 56                	push   %r14
 114:	41 55                	push   %r13
 116:	41 54                	push   %r12
 118:	53                   	push   %rbx
 119:	48 83 ec 20          	sub    $0x20,%rsp
 11d:	48 8d 3d 00 00 00 00 	lea    0x0(%rip),%rdi        # 124 <main+0x14>
 124:	e8 00 00 00 00       	call   129 <main+0x19>
 129:	48 89 c3             	mov    %rax,%rbx
 12c:	0f 57 c0             	xorps  %xmm0,%xmm0
 12f:	0f 57 c9             	xorps  %xmm1,%xmm1
 132:	31 c0                	xor    %eax,%eax
 134:	f3 0f 10 15 00 00 00 	movss  0x0(%rip),%xmm2        # 13c <main+0x2c>
 13b:	00 
 13c:	0f 28 1d 00 00 00 00 	movaps 0x0(%rip),%xmm3        # 143 <main+0x33>
 143:	49 be a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%r14
 14a:	24 03 e5 
 14d:	49 bf 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%r15
 154:	6b 00 00 
 157:	49 bc 94 57 53 fe 5a 	movabs $0x35afe535794,%r12
 15e:	03 00 00 
 161:	eb 24                	jmp    187 <main+0x77>
 163:	66 66 66 66 2e 0f 1f 	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
 16a:	84 00 00 00 00 00 
 170:	0f 28 e1             	movaps %xmm1,%xmm4
 173:	49 89 c5             	mov    %rax,%r13
 176:	0f 28 e8             	movaps %xmm0,%xmm5
 179:	0f 28 cc             	movaps %xmm4,%xmm1
 17c:	4c 89 e8             	mov    %r13,%rax
 17f:	0f 28 c5             	movaps %xmm5,%xmm0
 182:	49 39 dd             	cmp    %rbx,%r13
 185:	74 5e                	je     1e5 <main+0xd5>
 187:	48 39 d8             	cmp    %rbx,%rax
 18a:	7d e4                	jge    170 <main+0x60>
 18c:	0f 28 e1             	movaps %xmm1,%xmm4
 18f:	f3 0f 58 e2          	addss  %xmm2,%xmm4
 193:	0f 28 e8             	movaps %xmm0,%xmm5
 196:	0f 58 eb             	addps  %xmm3,%xmm5
 199:	4c 8d 68 01          	lea    0x1(%rax),%r13
 19d:	49 0f af c6          	imul   %r14,%rax
 1a1:	4c 01 f8             	add    %r15,%rax
 1a4:	48 c1 c8 06          	ror    $0x6,%rax
 1a8:	4c 39 e0             	cmp    %r12,%rax
 1ab:	77 cc                	ja     179 <main+0x69>
 1ad:	f3 0f 58 c8          	addss  %xmm0,%xmm1
 1b1:	0f c6 c0 55          	shufps $0x55,%xmm0,%xmm0
 1b5:	f3 0f 58 c1          	addss  %xmm1,%xmm0
 1b9:	f3 0f 11 64 24 0c    	movss  %xmm4,0xc(%rsp)
 1bf:	0f 29 6c 24 10       	movaps %xmm5,0x10(%rsp)
 1c4:	e8 00 00 00 00       	call   1c9 <main+0xb9>
 1c9:	0f 28 6c 24 10       	movaps 0x10(%rsp),%xmm5
 1ce:	f3 0f 10 64 24 0c    	movss  0xc(%rsp),%xmm4
 1d4:	0f 28 1d 00 00 00 00 	movaps 0x0(%rip),%xmm3        # 1db <main+0xcb>
 1db:	f3 0f 10 15 00 00 00 	movss  0x0(%rip),%xmm2        # 1e3 <main+0xd3>
 1e2:	00 
 1e3:	eb 94                	jmp    179 <main+0x69>
 1e5:	31 c0                	xor    %eax,%eax
 1e7:	48 83 c4 20          	add    $0x20,%rsp
 1eb:	5b                   	pop    %rbx
 1ec:	41 5c                	pop    %r12
 1ee:	41 5d                	pop    %r13
 1f0:	41 5e                	pop    %r14
 1f2:	41 5f                	pop    %r15
 1f4:	c3                   	ret
