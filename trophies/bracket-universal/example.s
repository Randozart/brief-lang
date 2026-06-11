
trophies/bracket-universal/example.o:     file format elf64-x86-64


Disassembly of section .text:

0000000000000000 <init_state>:
   0:	53                   	push   %rbx
   1:	48 89 fb             	mov    %rdi,%rbx
   4:	48 c7 07 c9 3c 00 00 	movq   $0x3cc9,(%rdi)
   b:	0f 57 c0             	xorps  %xmm0,%xmm0
   e:	0f 11 47 08          	movups %xmm0,0x8(%rdi)
  12:	48 c7 47 18 00 00 00 	movq   $0x0,0x18(%rdi)
  19:	00 
  1a:	31 ff                	xor    %edi,%edi
  1c:	e8 00 00 00 00       	call   21 <init_state+0x21>
  21:	88 43 20             	mov    %al,0x20(%rbx)
  24:	31 ff                	xor    %edi,%edi
  26:	e8 00 00 00 00       	call   2b <init_state+0x2b>
  2b:	88 43 21             	mov    %al,0x21(%rbx)
  2e:	31 ff                	xor    %edi,%edi
  30:	e8 00 00 00 00       	call   35 <init_state+0x35>
  35:	88 43 22             	mov    %al,0x22(%rbx)
  38:	5b                   	pop    %rbx
  39:	c3                   	ret
  3a:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

0000000000000040 <reactor_tick>:
  40:	c3                   	ret
  41:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
  48:	0f 1f 84 00 00 00 00 
  4f:	00 

0000000000000050 <main>:
  50:	50                   	push   %rax
  51:	31 ff                	xor    %edi,%edi
  53:	e8 00 00 00 00       	call   58 <main+0x8>
  58:	31 ff                	xor    %edi,%edi
  5a:	e8 00 00 00 00       	call   5f <main+0xf>
  5f:	31 ff                	xor    %edi,%edi
  61:	e8 00 00 00 00       	call   66 <main+0x16>
  66:	66 2e 0f 1f 84 00 00 	cs nopw 0x0(%rax,%rax,1)
  6d:	00 00 00 
  70:	eb fe                	jmp    70 <main+0x20>
