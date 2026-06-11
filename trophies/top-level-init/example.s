
trophies/top-level-init/example.o:     file format elf64-x86-64


Disassembly of section .text:

0000000000000000 <__init>:
   0:	53                   	push   %rbx
   1:	48 89 fb             	mov    %rdi,%rbx
   4:	48 8b 7f 08          	mov    0x8(%rdi),%rdi
   8:	e8 00 00 00 00       	call   d <__init+0xd>
   d:	48 c7 43 08 2a 00 00 	movq   $0x2a,0x8(%rbx)
  14:	00 
  15:	bf 2a 00 00 00       	mov    $0x2a,%edi
  1a:	e8 00 00 00 00       	call   1f <__init+0x1f>
  1f:	48 c7 03 01 00 00 00 	movq   $0x1,(%rbx)
  26:	5b                   	pop    %rbx
  27:	c3                   	ret
  28:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
  2f:	00 

0000000000000030 <init_state>:
  30:	0f 57 c0             	xorps  %xmm0,%xmm0
  33:	0f 11 07             	movups %xmm0,(%rdi)
  36:	c3                   	ret
  37:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
  3e:	00 00 

0000000000000040 <main>:
  40:	50                   	push   %rax
  41:	31 ff                	xor    %edi,%edi
  43:	e8 00 00 00 00       	call   48 <main+0x8>
  48:	31 ff                	xor    %edi,%edi
  4a:	e8 00 00 00 00       	call   4f <main+0xf>
  4f:	90                   	nop
  50:	eb fe                	jmp    50 <main+0x10>
