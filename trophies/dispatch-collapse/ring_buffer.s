
trophies/dispatch-collapse/ring_buffer.o:     file format elf64-x86-64


Disassembly of section .text:

0000000000000000 <work>:
   0:	48 89 f8             	mov    %rdi,%rax
   3:	48 8b 3f             	mov    (%rdi),%rdi
   6:	48 ff c7             	inc    %rdi
   9:	48 89 38             	mov    %rdi,(%rax)
   c:	48 b8 a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%rax
  13:	24 03 e5 
  16:	48 0f af c7          	imul   %rdi,%rax
  1a:	48 b9 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%rcx
  21:	6b 00 00 
  24:	48 01 c1             	add    %rax,%rcx
  27:	48 c1 c9 06          	ror    $0x6,%rcx
  2b:	48 b8 94 57 53 fe 5a 	movabs $0x35afe535794,%rax
  32:	03 00 00 
  35:	48 39 c1             	cmp    %rax,%rcx
  38:	0f 86 00 00 00 00    	jbe    3e <work+0x3e>
  3e:	c3                   	ret
  3f:	90                   	nop

0000000000000040 <init_state>:
  40:	53                   	push   %rbx
  41:	48 89 fb             	mov    %rdi,%rbx
  44:	48 c7 07 00 00 00 00 	movq   $0x0,(%rdi)
  4b:	48 8d 3d 00 00 00 00 	lea    0x0(%rip),%rdi        # 52 <init_state+0x12>
  52:	e8 00 00 00 00       	call   57 <init_state+0x17>
  57:	48 89 43 08          	mov    %rax,0x8(%rbx)
  5b:	5b                   	pop    %rbx
  5c:	c3                   	ret
  5d:	0f 1f 00             	nopl   (%rax)

0000000000000060 <main>:
  60:	41 57                	push   %r15
  62:	41 56                	push   %r14
  64:	41 55                	push   %r13
  66:	41 54                	push   %r12
  68:	53                   	push   %rbx
  69:	48 8d 3d 00 00 00 00 	lea    0x0(%rip),%rdi        # 70 <main+0x10>
  70:	e8 00 00 00 00       	call   75 <main+0x15>
  75:	48 89 c3             	mov    %rax,%rbx
  78:	31 ff                	xor    %edi,%edi
  7a:	49 be a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%r14
  81:	24 03 e5 
  84:	49 bf 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%r15
  8b:	6b 00 00 
  8e:	49 bc 94 57 53 fe 5a 	movabs $0x35afe535794,%r12
  95:	03 00 00 
  98:	eb 11                	jmp    ab <main+0x4b>
  9a:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)
  a0:	49 89 fd             	mov    %rdi,%r13
  a3:	4c 89 ef             	mov    %r13,%rdi
  a6:	49 39 dd             	cmp    %rbx,%r13
  a9:	74 23                	je     ce <main+0x6e>
  ab:	48 39 df             	cmp    %rbx,%rdi
  ae:	7d f0                	jge    a0 <main+0x40>
  b0:	4c 8d 6f 01          	lea    0x1(%rdi),%r13
  b4:	48 89 f8             	mov    %rdi,%rax
  b7:	49 0f af c6          	imul   %r14,%rax
  bb:	4c 01 f8             	add    %r15,%rax
  be:	48 c1 c8 06          	ror    $0x6,%rax
  c2:	4c 39 e0             	cmp    %r12,%rax
  c5:	77 dc                	ja     a3 <main+0x43>
  c7:	e8 00 00 00 00       	call   cc <main+0x6c>
  cc:	eb d5                	jmp    a3 <main+0x43>
  ce:	31 c0                	xor    %eax,%eax
  d0:	5b                   	pop    %rbx
  d1:	41 5c                	pop    %r12
  d3:	41 5d                	pop    %r13
  d5:	41 5e                	pop    %r14
  d7:	41 5f                	pop    %r15
  d9:	c3                   	ret
