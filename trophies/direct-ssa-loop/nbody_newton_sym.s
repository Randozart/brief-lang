
trophies/direct-ssa-loop/nbody_newton_sym.o:     file format elf64-x86-64


Disassembly of section .text:

0000000000000000 <simulate>:
       0:	53                   	push   %rbx
       1:	48 81 ec a0 01 00 00 	sub    $0x1a0,%rsp
       8:	48 89 fb             	mov    %rdi,%rbx
       b:	c5 fa 10 6f 30       	vmovss 0x30(%rdi),%xmm5
      10:	c5 fa 10 47 48       	vmovss 0x48(%rdi),%xmm0
      15:	c5 f8 29 44 24 10    	vmovaps %xmm0,0x10(%rsp)
      1b:	c5 fa 10 5f 60       	vmovss 0x60(%rdi),%xmm3
      20:	c5 f8 29 9c 24 c0 00 	vmovaps %xmm3,0xc0(%rsp)
      27:	00 00 
      29:	c5 fa 10 67 78       	vmovss 0x78(%rdi),%xmm4
      2e:	c5 fa 5c c3          	vsubss %xmm3,%xmm0,%xmm0
      32:	c5 fa 59 c8          	vmulss %xmm0,%xmm0,%xmm1
      36:	c4 e3 51 21 d3 10    	vinsertps $0x10,%xmm3,%xmm5,%xmm2
      3c:	c5 78 28 dd          	vmovaps %xmm5,%xmm11
      40:	c5 f8 29 ac 24 90 00 	vmovaps %xmm5,0x90(%rsp)
      47:	00 00 
      49:	c4 e3 61 21 dc 10    	vinsertps $0x10,%xmm4,%xmm3,%xmm3
      4f:	c5 78 28 c4          	vmovaps %xmm4,%xmm8
      53:	c5 e8 5c d3          	vsubps %xmm3,%xmm2,%xmm2
      57:	c5 f8 29 54 24 30    	vmovaps %xmm2,0x30(%rsp)
      5d:	c5 fb 10 57 40       	vmovsd 0x40(%rdi),%xmm2
      62:	c5 f8 29 54 24 60    	vmovaps %xmm2,0x60(%rsp)
      68:	c5 fb 10 5f 58       	vmovsd 0x58(%rdi),%xmm3
      6d:	c5 f8 29 5c 24 50    	vmovaps %xmm3,0x50(%rsp)
      73:	c5 e8 5c d3          	vsubps %xmm3,%xmm2,%xmm2
      77:	c5 ea 59 da          	vmulss %xmm2,%xmm2,%xmm3
      7b:	c5 e2 58 c9          	vaddss %xmm1,%xmm3,%xmm1
      7f:	c5 fa 16 ea          	vmovshdup %xmm2,%xmm5
      83:	c5 d2 59 dd          	vmulss %xmm5,%xmm5,%xmm3
      87:	c5 e2 58 c9          	vaddss %xmm1,%xmm3,%xmm1
      8b:	c5 fa 10 35 00 00 00 	vmovss 0x0(%rip),%xmm6        # 93 <simulate+0x93>
      92:	00 
      93:	c5 f2 59 de          	vmulss %xmm6,%xmm1,%xmm3
      97:	c5 78 28 ce          	vmovaps %xmm6,%xmm9
      9b:	c5 fa 10 35 00 00 00 	vmovss 0x0(%rip),%xmm6        # a3 <simulate+0xa3>
      a2:	00 
      a3:	c5 e2 58 de          	vaddss %xmm6,%xmm3,%xmm3
      a7:	c5 78 28 d6          	vmovaps %xmm6,%xmm10
      ab:	c5 f2 5e f3          	vdivss %xmm3,%xmm1,%xmm6
      af:	c5 ca 58 f3          	vaddss %xmm3,%xmm6,%xmm6
      b3:	c5 fa 10 1d 00 00 00 	vmovss 0x0(%rip),%xmm3        # bb <simulate+0xbb>
      ba:	00 
      bb:	c5 ca 59 f3          	vmulss %xmm3,%xmm6,%xmm6
      bf:	c5 f2 5e fe          	vdivss %xmm6,%xmm1,%xmm7
      c3:	c5 c2 58 f6          	vaddss %xmm6,%xmm7,%xmm6
      c7:	c5 ca 59 f3          	vmulss %xmm3,%xmm6,%xmm6
      cb:	c5 f2 5e fe          	vdivss %xmm6,%xmm1,%xmm7
      cf:	c5 c2 58 f6          	vaddss %xmm6,%xmm7,%xmm6
      d3:	c5 ca 59 f3          	vmulss %xmm3,%xmm6,%xmm6
      d7:	c5 f2 5e fe          	vdivss %xmm6,%xmm1,%xmm7
      db:	c5 c2 58 f6          	vaddss %xmm6,%xmm7,%xmm6
      df:	c5 f2 59 cb          	vmulss %xmm3,%xmm1,%xmm1
      e3:	c5 78 28 fb          	vmovaps %xmm3,%xmm15
      e7:	c5 f2 59 ce          	vmulss %xmm6,%xmm1,%xmm1
      eb:	c5 fa 10 3d 00 00 00 	vmovss 0x0(%rip),%xmm7        # f3 <simulate+0xf3>
      f2:	00 
      f3:	c5 c2 5e f1          	vdivss %xmm1,%xmm7,%xmm6
      f7:	c5 f8 28 e7          	vmovaps %xmm7,%xmm4
      fb:	c5 ca 59 0d 00 00 00 	vmulss 0x0(%rip),%xmm6,%xmm1        # 103 <simulate+0x103>
     102:	00 
     103:	c5 fa 12 f9          	vmovsldup %xmm1,%xmm7
     107:	c5 c0 59 da          	vmulps %xmm2,%xmm7,%xmm3
     10b:	c5 f8 29 9c 24 40 01 	vmovaps %xmm3,0x140(%rsp)
     112:	00 00 
     114:	c5 f2 59 c8          	vmulss %xmm0,%xmm1,%xmm1
     118:	c5 fa 11 8c 24 50 01 	vmovss %xmm1,0x150(%rsp)
     11f:	00 00 
     121:	c5 ca 59 35 00 00 00 	vmulss 0x0(%rip),%xmm6,%xmm6        # 129 <simulate+0x129>
     128:	00 
     129:	c5 ca 59 ca          	vmulss %xmm2,%xmm6,%xmm1
     12d:	c5 fa 11 4c 24 04    	vmovss %xmm1,0x4(%rsp)
     133:	c5 ca 59 d5          	vmulss %xmm5,%xmm6,%xmm2
     137:	c5 ca 59 c0          	vmulss %xmm0,%xmm6,%xmm0
     13b:	c4 e3 69 21 c0 10    	vinsertps $0x10,%xmm0,%xmm2,%xmm0
     141:	c5 f8 29 84 24 60 01 	vmovaps %xmm0,0x160(%rsp)
     148:	00 00 
     14a:	c5 fb 10 5f 70       	vmovsd 0x70(%rdi),%xmm3
     14f:	c5 fb 12 47 28       	vmovddup 0x28(%rdi),%xmm0
     154:	c5 f8 5c d3          	vsubps %xmm3,%xmm0,%xmm2
     158:	c5 e8 59 ea          	vmulps %xmm2,%xmm2,%xmm5
     15c:	c5 fa 16 f5          	vmovshdup %xmm5,%xmm6
     160:	c5 ca 58 ed          	vaddss %xmm5,%xmm6,%xmm5
     164:	c5 78 29 c1          	vmovaps %xmm8,%xmm1
     168:	c5 78 29 84 24 b0 00 	vmovaps %xmm8,0xb0(%rsp)
     16f:	00 00 
     171:	c4 41 22 5c c0       	vsubss %xmm8,%xmm11,%xmm8
     176:	c4 c1 3a 59 f0       	vmulss %xmm8,%xmm8,%xmm6
     17b:	c5 d2 58 ee          	vaddss %xmm6,%xmm5,%xmm5
     17f:	c5 b2 59 f5          	vmulss %xmm5,%xmm9,%xmm6
     183:	c4 41 78 28 d9       	vmovaps %xmm9,%xmm11
     188:	c5 aa 58 f6          	vaddss %xmm6,%xmm10,%xmm6
     18c:	c5 d2 5e fe          	vdivss %xmm6,%xmm5,%xmm7
     190:	c5 c2 58 f6          	vaddss %xmm6,%xmm7,%xmm6
     194:	c5 82 59 f6          	vmulss %xmm6,%xmm15,%xmm6
     198:	c5 d2 5e fe          	vdivss %xmm6,%xmm5,%xmm7
     19c:	c5 c2 58 f6          	vaddss %xmm6,%xmm7,%xmm6
     1a0:	c5 82 59 f6          	vmulss %xmm6,%xmm15,%xmm6
     1a4:	c5 d2 5e fe          	vdivss %xmm6,%xmm5,%xmm7
     1a8:	c5 c2 58 f6          	vaddss %xmm6,%xmm7,%xmm6
     1ac:	c5 82 59 f6          	vmulss %xmm6,%xmm15,%xmm6
     1b0:	c5 d2 5e fe          	vdivss %xmm6,%xmm5,%xmm7
     1b4:	c5 c2 58 f6          	vaddss %xmm6,%xmm7,%xmm6
     1b8:	c5 82 59 ed          	vmulss %xmm5,%xmm15,%xmm5
     1bc:	c5 d2 59 ee          	vmulss %xmm6,%xmm5,%xmm5
     1c0:	c5 da 5e f5          	vdivss %xmm5,%xmm4,%xmm6
     1c4:	c5 fa 10 25 00 00 00 	vmovss 0x0(%rip),%xmm4        # 1cc <simulate+0x1cc>
     1cb:	00 
     1cc:	c5 4a 59 cc          	vmulss %xmm4,%xmm6,%xmm9
     1d0:	c4 c1 7a 12 e9       	vmovsldup %xmm9,%xmm5
     1d5:	c5 d0 59 ea          	vmulps %xmm2,%xmm5,%xmm5
     1d9:	c5 f8 29 ac 24 30 01 	vmovaps %xmm5,0x130(%rsp)
     1e0:	00 00 
     1e2:	c5 4a 59 25 00 00 00 	vmulss 0x0(%rip),%xmm6,%xmm12        # 1ea <simulate+0x1ea>
     1e9:	00 
     1ea:	c4 c1 7a 12 f4       	vmovsldup %xmm12,%xmm6
     1ef:	c5 c8 59 fa          	vmulps %xmm2,%xmm6,%xmm7
     1f3:	c4 c1 32 59 f0       	vmulss %xmm8,%xmm9,%xmm6
     1f8:	c4 c1 1a 59 d0       	vmulss %xmm8,%xmm12,%xmm2
     1fd:	c4 e2 79 18 6f 18    	vbroadcastss 0x18(%rdi),%xmm5
     203:	c5 f8 29 ac 24 80 00 	vmovaps %xmm5,0x80(%rsp)
     20a:	00 00 
     20c:	c5 52 5c c1          	vsubss %xmm1,%xmm5,%xmm8
     210:	c4 41 3a 59 c8       	vmulss %xmm8,%xmm8,%xmm9
     215:	c5 fb 10 4f 10       	vmovsd 0x10(%rdi),%xmm1
     21a:	c5 f8 29 4c 24 70    	vmovaps %xmm1,0x70(%rsp)
     220:	c5 70 5c e3          	vsubps %xmm3,%xmm1,%xmm12
     224:	c5 f8 29 9c 24 a0 00 	vmovaps %xmm3,0xa0(%rsp)
     22b:	00 00 
     22d:	c4 41 1a 59 ec       	vmulss %xmm12,%xmm12,%xmm13
     232:	c4 41 12 58 c9       	vaddss %xmm9,%xmm13,%xmm9
     237:	c4 41 7a 16 ec       	vmovshdup %xmm12,%xmm13
     23c:	c4 41 12 59 f5       	vmulss %xmm13,%xmm13,%xmm14
     241:	c4 41 0a 58 c9       	vaddss %xmm9,%xmm14,%xmm9
     246:	c4 41 32 59 f3       	vmulss %xmm11,%xmm9,%xmm14
     24b:	c4 41 0a 58 f2       	vaddss %xmm10,%xmm14,%xmm14
     250:	c4 c1 32 5e ce       	vdivss %xmm14,%xmm9,%xmm1
     255:	c5 8a 58 c9          	vaddss %xmm1,%xmm14,%xmm1
     259:	c5 82 59 c9          	vmulss %xmm1,%xmm15,%xmm1
     25d:	c5 32 5e f1          	vdivss %xmm1,%xmm9,%xmm14
     261:	c5 8a 58 c9          	vaddss %xmm1,%xmm14,%xmm1
     265:	c5 82 59 c9          	vmulss %xmm1,%xmm15,%xmm1
     269:	c5 32 5e f1          	vdivss %xmm1,%xmm9,%xmm14
     26d:	c5 8a 58 c9          	vaddss %xmm1,%xmm14,%xmm1
     271:	c5 82 59 c9          	vmulss %xmm1,%xmm15,%xmm1
     275:	c5 32 5e f1          	vdivss %xmm1,%xmm9,%xmm14
     279:	c5 8a 58 c9          	vaddss %xmm1,%xmm14,%xmm1
     27d:	c4 41 32 59 cf       	vmulss %xmm15,%xmm9,%xmm9
     282:	c5 b2 59 c9          	vmulss %xmm1,%xmm9,%xmm1
     286:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # 28e <simulate+0x28e>
     28d:	00 
     28e:	c5 d2 5e c9          	vdivss %xmm1,%xmm5,%xmm1
     292:	c5 72 59 cc          	vmulss %xmm4,%xmm1,%xmm9
     296:	c4 41 32 59 ed       	vmulss %xmm13,%xmm9,%xmm13
     29b:	c4 41 32 59 f0       	vmulss %xmm8,%xmm9,%xmm14
     2a0:	c4 c3 11 21 e6 10    	vinsertps $0x10,%xmm14,%xmm13,%xmm4
     2a6:	c5 f8 29 a4 24 20 01 	vmovaps %xmm4,0x120(%rsp)
     2ad:	00 00 
     2af:	c4 c1 32 59 e4       	vmulss %xmm12,%xmm9,%xmm4
     2b4:	c5 fa 11 64 24 40    	vmovss %xmm4,0x40(%rsp)
     2ba:	c5 f2 59 0d 00 00 00 	vmulss 0x0(%rip),%xmm1,%xmm1        # 2c2 <simulate+0x2c2>
     2c1:	00 
     2c2:	c5 7a 12 c9          	vmovsldup %xmm1,%xmm9
     2c6:	c4 41 30 59 cc       	vmulps %xmm12,%xmm9,%xmm9
     2cb:	c5 7b 10 67 7c       	vmovsd 0x7c(%rdi),%xmm12
     2d0:	c4 41 18 58 c9       	vaddps %xmm9,%xmm12,%xmm9
     2d5:	c5 b0 58 ff          	vaddps %xmm7,%xmm9,%xmm7
     2d9:	c5 ba 59 c9          	vmulss %xmm1,%xmm8,%xmm1
     2dd:	c5 f2 58 8f 84 00 00 	vaddss 0x84(%rdi),%xmm1,%xmm1
     2e4:	00 
     2e5:	c5 7b 10 47 68       	vmovsd 0x68(%rdi),%xmm8
     2ea:	c5 b8 16 e7          	vmovlhps %xmm7,%xmm8,%xmm4
     2ee:	c5 f8 29 a4 24 10 01 	vmovaps %xmm4,0x110(%rsp)
     2f5:	00 00 
     2f7:	c5 f2 58 d2          	vaddss %xmm2,%xmm1,%xmm2
     2fb:	c5 f8 5c 7c 24 60    	vsubps 0x60(%rsp),%xmm0,%xmm7
     301:	c5 c0 59 cf          	vmulps %xmm7,%xmm7,%xmm1
     305:	c5 7a 16 c1          	vmovshdup %xmm1,%xmm8
     309:	c5 ba 58 c9          	vaddss %xmm1,%xmm8,%xmm1
     30d:	c5 f8 28 a4 24 90 00 	vmovaps 0x90(%rsp),%xmm4
     314:	00 00 
     316:	c5 5a 5c 44 24 10    	vsubss 0x10(%rsp),%xmm4,%xmm8
     31c:	c4 41 3a 59 c8       	vmulss %xmm8,%xmm8,%xmm9
     321:	c5 b2 58 c9          	vaddss %xmm1,%xmm9,%xmm1
     325:	c5 22 59 c9          	vmulss %xmm1,%xmm11,%xmm9
     329:	c4 41 32 58 ca       	vaddss %xmm10,%xmm9,%xmm9
     32e:	c4 41 72 5e e1       	vdivss %xmm9,%xmm1,%xmm12
     333:	c4 41 1a 58 c9       	vaddss %xmm9,%xmm12,%xmm9
     338:	c4 41 32 59 cf       	vmulss %xmm15,%xmm9,%xmm9
     33d:	c4 41 72 5e e1       	vdivss %xmm9,%xmm1,%xmm12
     342:	c4 41 1a 58 c9       	vaddss %xmm9,%xmm12,%xmm9
     347:	c4 41 32 59 cf       	vmulss %xmm15,%xmm9,%xmm9
     34c:	c4 41 72 5e e1       	vdivss %xmm9,%xmm1,%xmm12
     351:	c4 41 1a 58 c9       	vaddss %xmm9,%xmm12,%xmm9
     356:	c4 41 32 59 cf       	vmulss %xmm15,%xmm9,%xmm9
     35b:	c4 41 72 5e e1       	vdivss %xmm9,%xmm1,%xmm12
     360:	c4 41 1a 58 c9       	vaddss %xmm9,%xmm12,%xmm9
     365:	c5 82 59 c9          	vmulss %xmm1,%xmm15,%xmm1
     369:	c5 b2 59 c9          	vmulss %xmm1,%xmm9,%xmm1
     36d:	c5 52 5e c9          	vdivss %xmm1,%xmm5,%xmm9
     371:	c5 32 59 2d 00 00 00 	vmulss 0x0(%rip),%xmm9,%xmm13        # 379 <simulate+0x379>
     378:	00 
     379:	c4 c1 12 59 c8       	vmulss %xmm8,%xmm13,%xmm1
     37e:	c5 f2 58 ce          	vaddss %xmm6,%xmm1,%xmm1
     382:	c5 b2 59 2d 00 00 00 	vmulss 0x0(%rip),%xmm9,%xmm5        # 38a <simulate+0x38a>
     389:	00 
     38a:	c5 fa 12 f5          	vmovsldup %xmm5,%xmm6
     38e:	c5 c8 59 f7          	vmulps %xmm7,%xmm6,%xmm6
     392:	c5 7b 10 4f 4c       	vmovsd 0x4c(%rdi),%xmm9
     397:	c5 b0 58 f6          	vaddps %xmm6,%xmm9,%xmm6
     39b:	c5 f8 29 b4 24 e0 00 	vmovaps %xmm6,0xe0(%rsp)
     3a2:	00 00 
     3a4:	c5 ba 59 ed          	vmulss %xmm5,%xmm8,%xmm5
     3a8:	c5 fa 11 ac 24 00 01 	vmovss %xmm5,0x100(%rsp)
     3af:	00 00 
     3b1:	c4 c1 7a 12 ed       	vmovsldup %xmm13,%xmm5
     3b6:	c5 50 59 f7          	vmulps %xmm7,%xmm5,%xmm14
     3ba:	c5 f8 28 6c 24 50    	vmovaps 0x50(%rsp),%xmm5
     3c0:	c5 f8 5c f5          	vsubps %xmm5,%xmm0,%xmm6
     3c4:	c5 d0 5c c3          	vsubps %xmm3,%xmm5,%xmm0
     3c8:	c4 e3 49 21 e8 1c    	vinsertps $0x1c,%xmm0,%xmm6,%xmm5
     3ce:	c5 d0 59 ed          	vmulps %xmm5,%xmm5,%xmm5
     3d2:	c5 f8 28 5c 24 30    	vmovaps 0x30(%rsp),%xmm3
     3d8:	c5 e0 59 fb          	vmulps %xmm3,%xmm3,%xmm7
     3dc:	c5 d0 58 ef          	vaddps %xmm7,%xmm5,%xmm5
     3e0:	c5 fa 16 fe          	vmovshdup %xmm6,%xmm7
     3e4:	c4 63 41 0c c0 02    	vblendps $0x2,%xmm0,%xmm7,%xmm8
     3ea:	c4 41 38 59 c0       	vmulps %xmm8,%xmm8,%xmm8
     3ef:	c5 38 58 c5          	vaddps %xmm5,%xmm8,%xmm8
     3f3:	c5 b8 59 2d 00 00 00 	vmulps 0x0(%rip),%xmm8,%xmm5        # 3fb <simulate+0x3fb>
     3fa:	00 
     3fb:	c5 d0 58 2d 00 00 00 	vaddps 0x0(%rip),%xmm5,%xmm5        # 403 <simulate+0x403>
     402:	00 
     403:	c5 78 53 cd          	vrcpps %xmm5,%xmm9
     407:	c4 41 38 59 e9       	vmulps %xmm9,%xmm8,%xmm13
     40c:	c5 10 59 e5          	vmulps %xmm5,%xmm13,%xmm12
     410:	c4 41 38 5c e4       	vsubps %xmm12,%xmm8,%xmm12
     415:	c4 41 30 59 cc       	vmulps %xmm12,%xmm9,%xmm9
     41a:	c5 90 58 ed          	vaddps %xmm5,%xmm13,%xmm5
     41e:	c5 30 58 cd          	vaddps %xmm5,%xmm9,%xmm9
     422:	c4 e2 79 18 2d 00 00 	vbroadcastss 0x0(%rip),%xmm5        # 42b <simulate+0x42b>
     429:	00 00 
     42b:	c5 30 59 cd          	vmulps %xmm5,%xmm9,%xmm9
     42f:	c4 41 78 53 e1       	vrcpps %xmm9,%xmm12
     434:	c4 41 38 59 ec       	vmulps %xmm12,%xmm8,%xmm13
     439:	c4 41 30 59 dd       	vmulps %xmm13,%xmm9,%xmm11
     43e:	c4 41 38 5c db       	vsubps %xmm11,%xmm8,%xmm11
     443:	c4 41 18 59 db       	vmulps %xmm11,%xmm12,%xmm11
     448:	c4 41 10 58 c9       	vaddps %xmm9,%xmm13,%xmm9
     44d:	c4 41 30 58 cb       	vaddps %xmm11,%xmm9,%xmm9
     452:	c5 30 59 cd          	vmulps %xmm5,%xmm9,%xmm9
     456:	c4 41 78 53 d9       	vrcpps %xmm9,%xmm11
     45b:	c4 41 38 59 e3       	vmulps %xmm11,%xmm8,%xmm12
     460:	c4 41 30 59 ec       	vmulps %xmm12,%xmm9,%xmm13
     465:	c4 41 38 5c ed       	vsubps %xmm13,%xmm8,%xmm13
     46a:	c4 41 20 59 dd       	vmulps %xmm13,%xmm11,%xmm11
     46f:	c4 41 18 58 c9       	vaddps %xmm9,%xmm12,%xmm9
     474:	c4 41 30 58 cb       	vaddps %xmm11,%xmm9,%xmm9
     479:	c5 30 59 cd          	vmulps %xmm5,%xmm9,%xmm9
     47d:	c4 41 78 53 d9       	vrcpps %xmm9,%xmm11
     482:	c4 41 38 59 e3       	vmulps %xmm11,%xmm8,%xmm12
     487:	c4 41 30 59 ec       	vmulps %xmm12,%xmm9,%xmm13
     48c:	c4 41 38 5c ed       	vsubps %xmm13,%xmm8,%xmm13
     491:	c4 41 20 59 dd       	vmulps %xmm13,%xmm11,%xmm11
     496:	c4 41 18 58 c9       	vaddps %xmm9,%xmm12,%xmm9
     49b:	c4 41 30 58 cb       	vaddps %xmm11,%xmm9,%xmm9
     4a0:	c5 38 59 c5          	vmulps %xmm5,%xmm8,%xmm8
     4a4:	c4 41 38 59 c1       	vmulps %xmm9,%xmm8,%xmm8
     4a9:	c4 41 78 53 c8       	vrcpps %xmm8,%xmm9
     4ae:	c4 e2 79 18 2d 00 00 	vbroadcastss 0x0(%rip),%xmm5        # 4b7 <simulate+0x4b7>
     4b5:	00 00 
     4b7:	c5 30 59 dd          	vmulps %xmm5,%xmm9,%xmm11
     4bb:	c4 41 38 59 c3       	vmulps %xmm11,%xmm8,%xmm8
     4c0:	c4 41 50 5c c0       	vsubps %xmm8,%xmm5,%xmm8
     4c5:	c4 41 30 59 c0       	vmulps %xmm8,%xmm9,%xmm8
     4ca:	c4 41 20 58 c0       	vaddps %xmm8,%xmm11,%xmm8
     4cf:	c4 41 7a 16 c8       	vmovshdup %xmm8,%xmm9
     4d4:	c5 32 59 0d 00 00 00 	vmulss 0x0(%rip),%xmm9,%xmm9        # 4dc <simulate+0x4dc>
     4db:	00 
     4dc:	c4 41 7a 12 d9       	vmovsldup %xmm9,%xmm11
     4e1:	c5 f8 28 eb          	vmovaps %xmm3,%xmm5
     4e5:	c4 63 61 21 e0 4c    	vinsertps $0x4c,%xmm0,%xmm3,%xmm12
     4eb:	c4 c1 20 59 dc       	vmulps %xmm12,%xmm11,%xmm3
     4f0:	c5 f8 29 5c 24 20    	vmovaps %xmm3,0x20(%rsp)
     4f6:	c5 7a 16 dd          	vmovshdup %xmm5,%xmm11
     4fa:	c5 38 59 25 00 00 00 	vmulps 0x0(%rip),%xmm8,%xmm12        # 502 <simulate+0x502>
     501:	00 
     502:	c4 41 7a 16 d4       	vmovshdup %xmm12,%xmm10
     507:	c4 41 2a 59 d3       	vmulss %xmm11,%xmm10,%xmm10
     50c:	c5 aa 58 d2          	vaddss %xmm2,%xmm10,%xmm2
     510:	c5 fa 11 54 24 0c    	vmovss %xmm2,0xc(%rsp)
     516:	c5 ba 59 15 00 00 00 	vmulss 0x0(%rip),%xmm8,%xmm2        # 51e <simulate+0x51e>
     51d:	00 
     51e:	c5 7a 12 c2          	vmovsldup %xmm2,%xmm8
     522:	c5 38 59 c6          	vmulps %xmm6,%xmm8,%xmm8
     526:	c4 c1 08 58 d8       	vaddps %xmm8,%xmm14,%xmm3
     52b:	c5 f8 29 9c 24 f0 00 	vmovaps %xmm3,0xf0(%rsp)
     532:	00 00 
     534:	c5 ea 59 d5          	vmulss %xmm5,%xmm2,%xmm2
     538:	c5 72 58 c2          	vaddss %xmm2,%xmm1,%xmm8
     53c:	c5 9a 59 ce          	vmulss %xmm6,%xmm12,%xmm1
     540:	c5 f2 58 4c 24 04    	vaddss 0x4(%rsp),%xmm1,%xmm1
     546:	c5 fa 11 4c 24 08    	vmovss %xmm1,0x8(%rsp)
     54c:	c5 c0 14 cd          	vunpcklps %xmm5,%xmm7,%xmm1
     550:	c5 b2 59 d0          	vmulss %xmm0,%xmm9,%xmm2
     554:	c5 fa 11 54 24 30    	vmovss %xmm2,0x30(%rsp)
     55a:	c5 f0 16 c0          	vmovlhps %xmm0,%xmm1,%xmm0
     55e:	c4 c1 18 c6 cc 50    	vshufps $0x50,%xmm12,%xmm12,%xmm1
     564:	c5 f0 59 f0          	vmulps %xmm0,%xmm1,%xmm6
     568:	c5 78 28 7c 24 10    	vmovaps 0x10(%rsp),%xmm15
     56e:	c4 c3 59 21 c7 10    	vinsertps $0x10,%xmm15,%xmm4,%xmm0
     574:	c5 78 28 b4 24 80 00 	vmovaps 0x80(%rsp),%xmm14
     57b:	00 00 
     57d:	c5 88 5c d8          	vsubps %xmm0,%xmm14,%xmm3
     581:	c5 78 28 6c 24 70    	vmovaps 0x70(%rsp),%xmm13
     587:	c5 92 5c 67 28       	vsubss 0x28(%rdi),%xmm13,%xmm4
     58c:	c5 f8 28 6c 24 60    	vmovaps 0x60(%rsp),%xmm5
     592:	c5 90 5c cd          	vsubps %xmm5,%xmm13,%xmm1
     596:	c5 fa 12 d1          	vmovsldup %xmm1,%xmm2
     59a:	c4 e3 69 0c d4 01    	vblendps $0x1,%xmm4,%xmm2,%xmm2
     5a0:	c5 e8 59 d2          	vmulps %xmm2,%xmm2,%xmm2
     5a4:	c5 e0 59 fb          	vmulps %xmm3,%xmm3,%xmm7
     5a8:	c5 e8 58 ff          	vaddps %xmm7,%xmm2,%xmm7
     5ac:	c4 c1 7a 16 d5       	vmovshdup %xmm13,%xmm2
     5b1:	c5 ea 5c 57 2c       	vsubss 0x2c(%rdi),%xmm2,%xmm2
     5b6:	c4 63 71 0c ca 01    	vblendps $0x1,%xmm2,%xmm1,%xmm9
     5bc:	c4 41 30 59 c9       	vmulps %xmm9,%xmm9,%xmm9
     5c1:	c5 b0 58 ff          	vaddps %xmm7,%xmm9,%xmm7
     5c5:	c5 40 59 0d 00 00 00 	vmulps 0x0(%rip),%xmm7,%xmm9        # 5cd <simulate+0x5cd>
     5cc:	00 
     5cd:	c5 30 58 0d 00 00 00 	vaddps 0x0(%rip),%xmm9,%xmm9        # 5d5 <simulate+0x5d5>
     5d4:	00 
     5d5:	c4 41 78 53 d1       	vrcpps %xmm9,%xmm10
     5da:	c5 28 59 df          	vmulps %xmm7,%xmm10,%xmm11
     5de:	c4 41 30 59 e3       	vmulps %xmm11,%xmm9,%xmm12
     5e3:	c4 41 40 5c e4       	vsubps %xmm12,%xmm7,%xmm12
     5e8:	c4 41 28 59 d4       	vmulps %xmm12,%xmm10,%xmm10
     5ed:	c4 41 20 58 c9       	vaddps %xmm9,%xmm11,%xmm9
     5f2:	c4 41 30 58 ca       	vaddps %xmm10,%xmm9,%xmm9
     5f7:	c4 e2 79 18 05 00 00 	vbroadcastss 0x0(%rip),%xmm0        # 600 <simulate+0x600>
     5fe:	00 00 
     600:	c5 30 59 c8          	vmulps %xmm0,%xmm9,%xmm9
     604:	c4 41 78 53 d1       	vrcpps %xmm9,%xmm10
     609:	c5 28 59 df          	vmulps %xmm7,%xmm10,%xmm11
     60d:	c4 41 30 59 e3       	vmulps %xmm11,%xmm9,%xmm12
     612:	c4 41 40 5c e4       	vsubps %xmm12,%xmm7,%xmm12
     617:	c4 41 28 59 d4       	vmulps %xmm12,%xmm10,%xmm10
     61c:	c4 41 20 58 c9       	vaddps %xmm9,%xmm11,%xmm9
     621:	c4 41 30 58 ca       	vaddps %xmm10,%xmm9,%xmm9
     626:	c5 30 59 c8          	vmulps %xmm0,%xmm9,%xmm9
     62a:	c4 41 78 53 d1       	vrcpps %xmm9,%xmm10
     62f:	c5 28 59 df          	vmulps %xmm7,%xmm10,%xmm11
     633:	c4 41 30 59 e3       	vmulps %xmm11,%xmm9,%xmm12
     638:	c4 41 40 5c e4       	vsubps %xmm12,%xmm7,%xmm12
     63d:	c4 41 28 59 d4       	vmulps %xmm12,%xmm10,%xmm10
     642:	c4 41 20 58 c9       	vaddps %xmm9,%xmm11,%xmm9
     647:	c4 41 30 58 ca       	vaddps %xmm10,%xmm9,%xmm9
     64c:	c5 30 59 c8          	vmulps %xmm0,%xmm9,%xmm9
     650:	c4 41 78 53 d1       	vrcpps %xmm9,%xmm10
     655:	c5 28 59 df          	vmulps %xmm7,%xmm10,%xmm11
     659:	c4 41 30 59 e3       	vmulps %xmm11,%xmm9,%xmm12
     65e:	c4 41 40 5c e4       	vsubps %xmm12,%xmm7,%xmm12
     663:	c4 41 28 59 d4       	vmulps %xmm12,%xmm10,%xmm10
     668:	c4 41 20 58 c9       	vaddps %xmm9,%xmm11,%xmm9
     66d:	c4 41 30 58 ca       	vaddps %xmm10,%xmm9,%xmm9
     672:	c5 c0 59 f8          	vmulps %xmm0,%xmm7,%xmm7
     676:	c5 b0 59 ff          	vmulps %xmm7,%xmm9,%xmm7
     67a:	c5 78 53 cf          	vrcpps %xmm7,%xmm9
     67e:	c4 e2 79 18 05 00 00 	vbroadcastss 0x0(%rip),%xmm0        # 687 <simulate+0x687>
     685:	00 00 
     687:	c5 30 59 d0          	vmulps %xmm0,%xmm9,%xmm10
     68b:	c5 a8 59 ff          	vmulps %xmm7,%xmm10,%xmm7
     68f:	c5 f8 5c ff          	vsubps %xmm7,%xmm0,%xmm7
     693:	c5 b0 59 ff          	vmulps %xmm7,%xmm9,%xmm7
     697:	c5 28 58 d7          	vaddps %xmm7,%xmm10,%xmm10
     69b:	c5 7a 10 25 00 00 00 	vmovss 0x0(%rip),%xmm12        # 6a3 <simulate+0x6a3>
     6a2:	00 
     6a3:	c4 41 2a 59 dc       	vmulss %xmm12,%xmm10,%xmm11
     6a8:	c5 a2 59 fb          	vmulss %xmm3,%xmm11,%xmm7
     6ac:	c5 c2 58 7f 3c       	vaddss 0x3c(%rdi),%xmm7,%xmm7
     6b1:	c5 c8 58 84 24 10 01 	vaddps 0x110(%rsp),%xmm6,%xmm0
     6b8:	00 00 
     6ba:	c5 f8 29 84 24 d0 00 	vmovaps %xmm0,0xd0(%rsp)
     6c1:	00 00 
     6c3:	c4 c1 42 5c f0       	vsubss %xmm8,%xmm7,%xmm6
     6c8:	c5 fa 11 74 24 04    	vmovss %xmm6,0x4(%rsp)
     6ce:	c5 a8 59 35 00 00 00 	vmulps 0x0(%rip),%xmm10,%xmm6        # 6d6 <simulate+0x6d6>
     6d5:	00 
     6d6:	c5 fa 16 fe          	vmovshdup %xmm6,%xmm7
     6da:	c5 c2 59 f9          	vmulss %xmm1,%xmm7,%xmm7
     6de:	c5 c2 58 44 24 40    	vaddss 0x40(%rsp),%xmm7,%xmm0
     6e4:	c5 fa 11 44 24 40    	vmovss %xmm0,0x40(%rsp)
     6ea:	c5 fa 16 f9          	vmovshdup %xmm1,%xmm7
     6ee:	c5 c0 14 fb          	vunpcklps %xmm3,%xmm7,%xmm7
     6f2:	c4 63 41 21 c4 20    	vinsertps $0x20,%xmm4,%xmm7,%xmm8
     6f8:	c5 4a 59 cc          	vmulss %xmm4,%xmm6,%xmm9
     6fc:	c4 e3 61 0c e2 01    	vblendps $0x1,%xmm2,%xmm3,%xmm4
     702:	c5 c8 59 e4          	vmulps %xmm4,%xmm6,%xmm4
     706:	c4 43 39 21 c3 30    	vinsertps $0x30,%xmm11,%xmm8,%xmm8
     70c:	c4 c1 48 c6 f3 c1    	vshufps $0xc1,%xmm11,%xmm6,%xmm6
     712:	c4 e3 49 21 d2 30    	vinsertps $0x30,%xmm2,%xmm6,%xmm2
     718:	c5 b8 59 d2          	vmulps %xmm2,%xmm8,%xmm2
     71c:	c5 d8 16 67 34       	vmovhps 0x34(%rdi),%xmm4,%xmm4
     721:	c5 d8 58 fa          	vaddps %xmm2,%xmm4,%xmm7
     725:	c4 c1 7a 16 d2       	vmovshdup %xmm10,%xmm2
     72a:	c5 9a 59 d2          	vmulss %xmm2,%xmm12,%xmm2
     72e:	c5 fa 12 e2          	vmovsldup %xmm2,%xmm4
     732:	c5 d8 59 c1          	vmulps %xmm1,%xmm4,%xmm0
     736:	c5 fa 16 cb          	vmovshdup %xmm3,%xmm1
     73a:	c5 ea 59 c9          	vmulss %xmm1,%xmm2,%xmm1
     73e:	c5 f2 58 4f 54       	vaddss 0x54(%rdi),%xmm1,%xmm1
     743:	c5 f8 58 a4 24 e0 00 	vaddps 0xe0(%rsp),%xmm0,%xmm4
     74a:	00 00 
     74c:	c5 f2 58 b4 24 00 01 	vaddss 0x100(%rsp),%xmm1,%xmm6
     753:	00 00 
     755:	c5 f8 28 54 24 50    	vmovaps 0x50(%rsp),%xmm2
     75b:	c5 fa 16 c2          	vmovshdup %xmm2,%xmm0
     75f:	c4 e3 79 21 84 24 c0 	vinsertps $0x10,0xc0(%rsp),%xmm0,%xmm0
     766:	00 00 00 10 
     76a:	c5 f9 14 84 24 a0 00 	vunpcklpd 0xa0(%rsp),%xmm0,%xmm0
     771:	00 00 
     773:	c4 c3 09 21 cd 4c    	vinsertps $0x4c,%xmm13,%xmm14,%xmm1
     779:	c5 f0 16 cd          	vmovlhps %xmm5,%xmm1,%xmm1
     77d:	c5 70 5c e0          	vsubps %xmm0,%xmm1,%xmm12
     781:	c5 90 5c d2          	vsubps %xmm2,%xmm13,%xmm2
     785:	c5 e8 59 c2          	vmulps %xmm2,%xmm2,%xmm0
     789:	c4 41 18 59 c4       	vmulps %xmm12,%xmm12,%xmm8
     78e:	c4 c3 79 21 c0 90    	vinsertps $0x90,%xmm8,%xmm0,%xmm0
     794:	c4 41 38 c6 c0 ec    	vshufps $0xec,%xmm8,%xmm8,%xmm8
     79a:	c5 b8 58 c0          	vaddps %xmm0,%xmm8,%xmm0
     79e:	c5 02 5c 84 24 b0 00 	vsubss 0xb0(%rsp),%xmm15,%xmm8
     7a5:	00 00 
     7a7:	c4 41 7a 16 d4       	vmovshdup %xmm12,%xmm10
     7ac:	c4 43 29 21 d0 10    	vinsertps $0x10,%xmm8,%xmm10,%xmm10
     7b2:	c4 41 28 59 d2       	vmulps %xmm10,%xmm10,%xmm10
     7b7:	c5 a8 58 c0          	vaddps %xmm0,%xmm10,%xmm0
     7bb:	c5 78 59 15 00 00 00 	vmulps 0x0(%rip),%xmm0,%xmm10        # 7c3 <simulate+0x7c3>
     7c2:	00 
     7c3:	c5 28 58 15 00 00 00 	vaddps 0x0(%rip),%xmm10,%xmm10        # 7cb <simulate+0x7cb>
     7ca:	00 
     7cb:	c4 41 78 53 da       	vrcpps %xmm10,%xmm11
     7d0:	c5 a0 59 c8          	vmulps %xmm0,%xmm11,%xmm1
     7d4:	c5 28 59 f1          	vmulps %xmm1,%xmm10,%xmm14
     7d8:	c4 41 78 5c f6       	vsubps %xmm14,%xmm0,%xmm14
     7dd:	c4 41 20 59 de       	vmulps %xmm14,%xmm11,%xmm11
     7e2:	c5 a8 58 c9          	vaddps %xmm1,%xmm10,%xmm1
     7e6:	c5 a0 58 c9          	vaddps %xmm1,%xmm11,%xmm1
     7ea:	c4 e2 79 18 1d 00 00 	vbroadcastss 0x0(%rip),%xmm3        # 7f3 <simulate+0x7f3>
     7f1:	00 00 
     7f3:	c5 f0 59 cb          	vmulps %xmm3,%xmm1,%xmm1
     7f7:	c5 78 53 d1          	vrcpps %xmm1,%xmm10
     7fb:	c5 28 59 d8          	vmulps %xmm0,%xmm10,%xmm11
     7ff:	c5 20 59 f1          	vmulps %xmm1,%xmm11,%xmm14
     803:	c4 41 78 5c f6       	vsubps %xmm14,%xmm0,%xmm14
     808:	c4 41 28 59 d6       	vmulps %xmm14,%xmm10,%xmm10
     80d:	c5 a0 58 c9          	vaddps %xmm1,%xmm11,%xmm1
     811:	c5 a8 58 c9          	vaddps %xmm1,%xmm10,%xmm1
     815:	c5 f0 59 cb          	vmulps %xmm3,%xmm1,%xmm1
     819:	c5 78 53 d1          	vrcpps %xmm1,%xmm10
     81d:	c5 28 59 d8          	vmulps %xmm0,%xmm10,%xmm11
     821:	c5 20 59 f1          	vmulps %xmm1,%xmm11,%xmm14
     825:	c4 41 78 5c f6       	vsubps %xmm14,%xmm0,%xmm14
     82a:	c4 41 28 59 d6       	vmulps %xmm14,%xmm10,%xmm10
     82f:	c5 a0 58 c9          	vaddps %xmm1,%xmm11,%xmm1
     833:	c5 a8 58 c9          	vaddps %xmm1,%xmm10,%xmm1
     837:	c5 f0 59 cb          	vmulps %xmm3,%xmm1,%xmm1
     83b:	c5 78 53 d1          	vrcpps %xmm1,%xmm10
     83f:	c5 28 59 d8          	vmulps %xmm0,%xmm10,%xmm11
     843:	c5 20 59 f1          	vmulps %xmm1,%xmm11,%xmm14
     847:	c4 41 78 5c f6       	vsubps %xmm14,%xmm0,%xmm14
     84c:	c4 41 28 59 d6       	vmulps %xmm14,%xmm10,%xmm10
     851:	c5 a0 58 c9          	vaddps %xmm1,%xmm11,%xmm1
     855:	c5 a8 58 c9          	vaddps %xmm1,%xmm10,%xmm1
     859:	c5 f8 59 c3          	vmulps %xmm3,%xmm0,%xmm0
     85d:	c5 f8 59 c1          	vmulps %xmm1,%xmm0,%xmm0
     861:	c5 f8 53 c8          	vrcpps %xmm0,%xmm1
     865:	c4 e2 79 18 1d 00 00 	vbroadcastss 0x0(%rip),%xmm3        # 86e <simulate+0x86e>
     86c:	00 00 
     86e:	c5 f0 59 eb          	vmulps %xmm3,%xmm1,%xmm5
     872:	c5 f8 59 c5          	vmulps %xmm5,%xmm0,%xmm0
     876:	c5 e0 5c c0          	vsubps %xmm0,%xmm3,%xmm0
     87a:	c5 f0 59 c0          	vmulps %xmm0,%xmm1,%xmm0
     87e:	c5 d0 58 c0          	vaddps %xmm0,%xmm5,%xmm0
     882:	c5 fa 16 c8          	vmovshdup %xmm0,%xmm1
     886:	c5 f2 59 0d 00 00 00 	vmulss 0x0(%rip),%xmm1,%xmm1        # 88e <simulate+0x88e>
     88d:	00 
     88e:	c5 fa 12 e9          	vmovsldup %xmm1,%xmm5
     892:	c4 41 19 c6 d4 01    	vshufpd $0x1,%xmm12,%xmm12,%xmm10
     898:	c5 a8 59 ed          	vmulps %xmm5,%xmm10,%xmm5
     89c:	c5 d0 58 ac 24 40 01 	vaddps 0x140(%rsp),%xmm5,%xmm5
     8a3:	00 00 
     8a5:	c5 58 5c d5          	vsubps %xmm5,%xmm4,%xmm10
     8a9:	c5 ba 59 c9          	vmulss %xmm1,%xmm8,%xmm1
     8ad:	c5 f2 58 8c 24 50 01 	vaddss 0x150(%rsp),%xmm1,%xmm1
     8b4:	00 00 
     8b6:	c5 4a 5c d9          	vsubss %xmm1,%xmm6,%xmm11
     8ba:	c5 f8 59 0d 00 00 00 	vmulps 0x0(%rip),%xmm0,%xmm1        # 8c2 <simulate+0x8c2>
     8c1:	00 
     8c2:	c5 fa 16 e1          	vmovshdup %xmm1,%xmm4
     8c6:	c5 ba 59 e4          	vmulss %xmm4,%xmm8,%xmm4
     8ca:	c5 5a 58 7c 24 0c    	vaddss 0xc(%rsp),%xmm4,%xmm15
     8d0:	c5 fa 59 25 00 00 00 	vmulss 0x0(%rip),%xmm0,%xmm4        # 8d8 <simulate+0x8d8>
     8d7:	00 
     8d8:	c5 da 59 c2          	vmulss %xmm2,%xmm4,%xmm0
     8dc:	c5 b2 58 c0          	vaddss %xmm0,%xmm9,%xmm0
     8e0:	c5 fa 58 6c 24 40    	vaddss 0x40(%rsp),%xmm0,%xmm5
     8e6:	c5 f2 59 c2          	vmulss %xmm2,%xmm1,%xmm0
     8ea:	c5 fa 58 47 64       	vaddss 0x64(%rdi),%xmm0,%xmm0
     8ef:	c5 fa 58 54 24 08    	vaddss 0x8(%rsp),%xmm0,%xmm2
     8f5:	c5 f0 c6 c1 50       	vshufps $0x50,%xmm1,%xmm1,%xmm0
     8fa:	c5 98 59 c0          	vmulps %xmm0,%xmm12,%xmm0
     8fe:	c5 f8 58 84 24 d0 00 	vaddps 0xd0(%rsp),%xmm0,%xmm0
     905:	00 00 
     907:	c5 fa 10 4f 1c       	vmovss 0x1c(%rdi),%xmm1
     90c:	c5 f2 5c dd          	vsubss %xmm5,%xmm1,%xmm3
     910:	c4 e3 19 21 cc 10    	vinsertps $0x10,%xmm4,%xmm12,%xmm1
     916:	c5 f1 14 8c 24 30 01 	vunpcklpd 0x130(%rsp),%xmm1,%xmm1
     91d:	00 00 
     91f:	c4 e3 19 0c e4 01    	vblendps $0x1,%xmm4,%xmm12,%xmm4
     925:	c5 d9 14 a4 24 f0 00 	vunpcklpd 0xf0(%rsp),%xmm4,%xmm4
     92c:	00 00 
     92e:	c5 f0 59 ec          	vmulps %xmm4,%xmm1,%xmm5
     932:	c5 f0 58 cc          	vaddps %xmm4,%xmm1,%xmm1
     936:	c5 6a 5c 74 24 30    	vsubss 0x30(%rsp),%xmm2,%xmm14
     93c:	c4 e3 71 0c cd 03    	vblendps $0x3,%xmm5,%xmm1,%xmm1
     942:	c5 d0 58 94 24 20 01 	vaddps 0x120(%rsp),%xmm5,%xmm2
     949:	00 00 
     94b:	c5 c0 5c f1          	vsubps %xmm1,%xmm7,%xmm6
     94f:	c5 c0 58 ca          	vaddps %xmm2,%xmm7,%xmm1
     953:	c4 62 79 18 2d 00 00 	vbroadcastss 0x0(%rip),%xmm13        # 95c <simulate+0x95c>
     95a:	00 00 
     95c:	c5 90 59 d6          	vmulps %xmm6,%xmm13,%xmm2
     960:	c4 e3 69 0c c9 03    	vblendps $0x3,%xmm1,%xmm2,%xmm1
     966:	c5 f8 10 57 20       	vmovups 0x20(%rdi),%xmm2
     96b:	c5 e8 5c f9          	vsubps %xmm1,%xmm2,%xmm7
     96f:	c5 e8 58 e1          	vaddps %xmm1,%xmm2,%xmm4
     973:	c5 fa 12 cf          	vmovsldup %xmm7,%xmm1
     977:	c5 f8 29 5c 24 40    	vmovaps %xmm3,0x40(%rsp)
     97d:	c4 e3 71 0c cb 01    	vblendps $0x1,%xmm3,%xmm1,%xmm1
     983:	c4 e2 79 18 2d 00 00 	vbroadcastss 0x0(%rip),%xmm5        # 98c <simulate+0x98c>
     98a:	00 00 
     98c:	c5 f0 59 cd          	vmulps %xmm5,%xmm1,%xmm1
     990:	c5 70 58 44 24 70    	vaddps 0x70(%rsp),%xmm1,%xmm8
     996:	c5 fa 16 d7          	vmovshdup %xmm7,%xmm2
     99a:	c5 f8 29 94 24 50 01 	vmovaps %xmm2,0x150(%rsp)
     9a1:	00 00 
     9a3:	c5 f8 29 bc 24 20 01 	vmovaps %xmm7,0x120(%rsp)
     9aa:	00 00 
     9ac:	c5 7a 10 0d 00 00 00 	vmovss 0x0(%rip),%xmm9        # 9b4 <simulate+0x9b4>
     9b3:	00 
     9b4:	c5 b2 59 d2          	vmulss %xmm2,%xmm9,%xmm2
     9b8:	c5 ea 58 94 24 80 00 	vaddss 0x80(%rsp),%xmm2,%xmm2
     9bf:	00 00 
     9c1:	48 8b 47 08          	mov    0x8(%rdi),%rax
     9c5:	c5 fa 11 5f 1c       	vmovss %xmm3,0x1c(%rdi)
     9ca:	c5 c9 c6 de 01       	vshufpd $0x1,%xmm6,%xmm6,%xmm3
     9cf:	c5 f9 29 9c 24 40 01 	vmovapd %xmm3,0x140(%rsp)
     9d6:	00 00 
     9d8:	c5 fa 11 5f 34       	vmovss %xmm3,0x34(%rdi)
     9dd:	c5 c8 c6 de ff       	vshufps $0xff,%xmm6,%xmm6,%xmm3
     9e2:	c5 f8 29 9c 24 30 01 	vmovaps %xmm3,0x130(%rsp)
     9e9:	00 00 
     9eb:	c5 fa 11 5f 38       	vmovss %xmm3,0x38(%rdi)
     9f0:	c5 fa 10 5c 24 04    	vmovss 0x4(%rsp),%xmm3
     9f6:	c5 fa 11 5f 3c       	vmovss %xmm3,0x3c(%rdi)
     9fb:	c5 78 29 94 24 00 01 	vmovaps %xmm10,0x100(%rsp)
     a02:	00 00 
     a04:	c5 78 13 57 4c       	vmovlps %xmm10,0x4c(%rdi)
     a09:	c4 41 78 28 e3       	vmovaps %xmm11,%xmm12
     a0e:	c5 7a 11 9c 24 10 01 	vmovss %xmm11,0x110(%rsp)
     a15:	00 00 
     a17:	c5 7a 11 5f 54       	vmovss %xmm11,0x54(%rdi)
     a1c:	c5 7a 11 77 64       	vmovss %xmm14,0x64(%rdi)
     a21:	c5 78 29 74 24 30    	vmovaps %xmm14,0x30(%rsp)
     a27:	c5 f9 c6 f0 01       	vshufpd $0x1,%xmm0,%xmm0,%xmm6
     a2c:	c5 f9 29 b4 24 80 00 	vmovapd %xmm6,0x80(%rsp)
     a33:	00 00 
     a35:	c5 fa 11 77 7c       	vmovss %xmm6,0x7c(%rdi)
     a3a:	c5 f8 c6 f0 ff       	vshufps $0xff,%xmm0,%xmm0,%xmm6
     a3f:	c5 f8 29 74 24 70    	vmovaps %xmm6,0x70(%rsp)
     a45:	c5 fa 11 b7 80 00 00 	vmovss %xmm6,0x80(%rdi)
     a4c:	00 
     a4d:	c4 41 78 28 df       	vmovaps %xmm15,%xmm11
     a52:	c5 7a 11 7c 24 0c    	vmovss %xmm15,0xc(%rsp)
     a58:	c5 7a 11 bf 84 00 00 	vmovss %xmm15,0x84(%rdi)
     a5f:	00 
     a60:	c5 78 13 47 10       	vmovlps %xmm8,0x10(%rdi)
     a65:	c5 fa 11 57 18       	vmovss %xmm2,0x18(%rdi)
     a6a:	c4 e3 59 0c f7 03    	vblendps $0x3,%xmm7,%xmm4,%xmm6
     a70:	c5 f8 11 77 20       	vmovups %xmm6,0x20(%rdi)
     a75:	c5 b2 59 f3          	vmulss %xmm3,%xmm9,%xmm6
     a79:	c5 ca 58 8c 24 90 00 	vaddss 0x90(%rsp),%xmm6,%xmm1
     a80:	00 00 
     a82:	c5 a8 59 f5          	vmulps %xmm5,%xmm10,%xmm6
     a86:	c5 78 28 fd          	vmovaps %xmm5,%xmm15
     a8a:	c5 c8 58 7c 24 60    	vaddps 0x60(%rsp),%xmm6,%xmm7
     a90:	c4 c1 1a 59 f1       	vmulss %xmm9,%xmm12,%xmm6
     a95:	c4 41 78 28 d1       	vmovaps %xmm9,%xmm10
     a9a:	c5 4a 58 64 24 10    	vaddss 0x10(%rsp),%xmm6,%xmm12
     aa0:	c5 f8 58 b4 24 60 01 	vaddps 0x160(%rsp),%xmm0,%xmm6
     aa7:	00 00 
     aa9:	c5 90 59 c0          	vmulps %xmm0,%xmm13,%xmm0
     aad:	c4 e3 79 0c c6 03    	vblendps $0x3,%xmm6,%xmm0,%xmm0
     ab3:	c5 f8 28 5c 24 20    	vmovaps 0x20(%rsp),%xmm3
     ab9:	c5 e1 14 ac 24 a0 00 	vunpcklpd 0xa0(%rsp),%xmm3,%xmm5
     ac0:	00 00 
     ac2:	c5 f8 5c f5          	vsubps %xmm5,%xmm0,%xmm6
     ac6:	c5 f8 58 ed          	vaddps %xmm5,%xmm0,%xmm5
     aca:	c5 f8 29 74 24 60    	vmovaps %xmm6,0x60(%rsp)
     ad0:	c5 fa 12 c6          	vmovsldup %xmm6,%xmm0
     ad4:	c4 c3 79 0c c6 01    	vblendps $0x1,%xmm14,%xmm0,%xmm0
     ada:	c5 80 59 c0          	vmulps %xmm0,%xmm15,%xmm0
     ade:	c5 78 58 4c 24 50    	vaddps 0x50(%rsp),%xmm0,%xmm9
     ae4:	c5 fa 16 c6          	vmovshdup %xmm6,%xmm0
     ae8:	c5 f8 29 44 24 10    	vmovaps %xmm0,0x10(%rsp)
     aee:	c5 aa 59 c0          	vmulss %xmm0,%xmm10,%xmm0
     af2:	c5 fa 58 9c 24 c0 00 	vaddss 0xc0(%rsp),%xmm0,%xmm3
     af9:	00 00 
     afb:	c5 78 28 e9          	vmovaps %xmm1,%xmm13
     aff:	c5 fa 11 8c 24 f0 00 	vmovss %xmm1,0xf0(%rsp)
     b06:	00 00 
     b08:	c5 fa 11 4f 30       	vmovss %xmm1,0x30(%rdi)
     b0d:	c5 f8 13 7f 40       	vmovlps %xmm7,0x40(%rdi)
     b12:	c5 7a 11 67 48       	vmovss %xmm12,0x48(%rdi)
     b17:	c5 78 29 e1          	vmovaps %xmm12,%xmm1
     b1b:	c5 7a 11 a4 24 e0 00 	vmovss %xmm12,0xe0(%rsp)
     b22:	00 00 
     b24:	c5 78 13 4f 58       	vmovlps %xmm9,0x58(%rdi)
     b29:	c5 fa 11 5f 60       	vmovss %xmm3,0x60(%rdi)
     b2e:	c5 fa 11 9c 24 d0 00 	vmovss %xmm3,0xd0(%rsp)
     b35:	00 00 
     b37:	c4 e3 51 0c c6 03    	vblendps $0x3,%xmm6,%xmm5,%xmm0
     b3d:	c5 f8 11 47 68       	vmovups %xmm0,0x68(%rdi)
     b42:	c4 c1 22 59 c2       	vmulss %xmm10,%xmm11,%xmm0
     b47:	c5 fa 58 84 24 b0 00 	vaddss 0xb0(%rsp),%xmm0,%xmm0
     b4e:	00 00 
     b50:	c5 fa 11 44 24 08    	vmovss %xmm0,0x8(%rsp)
     b56:	c5 d9 c6 c4 01       	vshufpd $0x1,%xmm4,%xmm4,%xmm0
     b5b:	c5 f9 29 44 24 20    	vmovapd %xmm0,0x20(%rsp)
     b61:	c5 ba 5c c0          	vsubss %xmm0,%xmm8,%xmm0
     b65:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
     b69:	c4 c1 6a 5c f5       	vsubss %xmm13,%xmm2,%xmm6
     b6e:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
     b72:	c5 fa 58 f6          	vaddss %xmm6,%xmm0,%xmm6
     b76:	c5 58 c6 ec ff       	vshufps $0xff,%xmm4,%xmm4,%xmm13
     b7b:	c4 c1 7a 16 c0       	vmovshdup %xmm8,%xmm0
     b80:	c4 c1 7a 5c e5       	vsubss %xmm13,%xmm0,%xmm4
     b85:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
     b89:	c5 da 58 e6          	vaddss %xmm6,%xmm4,%xmm4
     b8d:	c5 7a 10 35 00 00 00 	vmovss 0x0(%rip),%xmm14        # b95 <simulate+0xb95>
     b94:	00 
     b95:	c5 8a 59 f4          	vmulss %xmm4,%xmm14,%xmm6
     b99:	c5 7a 10 25 00 00 00 	vmovss 0x0(%rip),%xmm12        # ba1 <simulate+0xba1>
     ba0:	00 
     ba1:	c5 9a 58 f6          	vaddss %xmm6,%xmm12,%xmm6
     ba5:	c5 5a 5e d6          	vdivss %xmm6,%xmm4,%xmm10
     ba9:	c5 aa 58 f6          	vaddss %xmm6,%xmm10,%xmm6
     bad:	c5 7a 10 3d 00 00 00 	vmovss 0x0(%rip),%xmm15        # bb5 <simulate+0xbb5>
     bb4:	00 
     bb5:	c5 82 59 f6          	vmulss %xmm6,%xmm15,%xmm6
     bb9:	c5 5a 5e d6          	vdivss %xmm6,%xmm4,%xmm10
     bbd:	c5 aa 58 f6          	vaddss %xmm6,%xmm10,%xmm6
     bc1:	c5 82 59 f6          	vmulss %xmm6,%xmm15,%xmm6
     bc5:	c5 5a 5e d6          	vdivss %xmm6,%xmm4,%xmm10
     bc9:	c5 aa 58 f6          	vaddss %xmm6,%xmm10,%xmm6
     bcd:	c5 82 59 f6          	vmulss %xmm6,%xmm15,%xmm6
     bd1:	c5 da 5e e6          	vdivss %xmm6,%xmm4,%xmm4
     bd5:	c5 da 58 e6          	vaddss %xmm6,%xmm4,%xmm4
     bd9:	c5 fa 11 64 24 50    	vmovss %xmm4,0x50(%rsp)
     bdf:	c5 ba 5c e7          	vsubss %xmm7,%xmm8,%xmm4
     be3:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
     be7:	c5 ea 5c f1          	vsubss %xmm1,%xmm2,%xmm6
     beb:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
     bef:	c5 da 58 e6          	vaddss %xmm6,%xmm4,%xmm4
     bf3:	c5 fa 16 cf          	vmovshdup %xmm7,%xmm1
     bf7:	c5 f8 29 bc 24 80 01 	vmovaps %xmm7,0x180(%rsp)
     bfe:	00 00 
     c00:	c5 fa 5c f1          	vsubss %xmm1,%xmm0,%xmm6
     c04:	c5 78 28 d1          	vmovaps %xmm1,%xmm10
     c08:	c5 f8 29 8c 24 70 01 	vmovaps %xmm1,0x170(%rsp)
     c0f:	00 00 
     c11:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
     c15:	c5 ca 58 e4          	vaddss %xmm4,%xmm6,%xmm4
     c19:	c5 8a 59 f4          	vmulss %xmm4,%xmm14,%xmm6
     c1d:	c4 41 78 28 de       	vmovaps %xmm14,%xmm11
     c22:	c5 9a 58 f6          	vaddss %xmm6,%xmm12,%xmm6
     c26:	c5 5a 5e f6          	vdivss %xmm6,%xmm4,%xmm14
     c2a:	c5 8a 58 f6          	vaddss %xmm6,%xmm14,%xmm6
     c2e:	c5 82 59 f6          	vmulss %xmm6,%xmm15,%xmm6
     c32:	c5 5a 5e f6          	vdivss %xmm6,%xmm4,%xmm14
     c36:	c5 8a 58 f6          	vaddss %xmm6,%xmm14,%xmm6
     c3a:	c5 82 59 f6          	vmulss %xmm6,%xmm15,%xmm6
     c3e:	c5 5a 5e f6          	vdivss %xmm6,%xmm4,%xmm14
     c42:	c5 8a 58 f6          	vaddss %xmm6,%xmm14,%xmm6
     c46:	c5 82 59 f6          	vmulss %xmm6,%xmm15,%xmm6
     c4a:	c5 da 5e e6          	vdivss %xmm6,%xmm4,%xmm4
     c4e:	c5 da 58 e6          	vaddss %xmm6,%xmm4,%xmm4
     c52:	c5 fa 11 a4 24 c0 00 	vmovss %xmm4,0xc0(%rsp)
     c59:	00 00 
     c5b:	c5 78 29 c9          	vmovaps %xmm9,%xmm1
     c5f:	c4 c1 3a 5c e1       	vsubss %xmm9,%xmm8,%xmm4
     c64:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
     c68:	c5 ea 5c f3          	vsubss %xmm3,%xmm2,%xmm6
     c6c:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
     c70:	c5 da 58 e6          	vaddss %xmm6,%xmm4,%xmm4
     c74:	c4 c1 7a 16 d9       	vmovshdup %xmm9,%xmm3
     c79:	c5 f8 29 9c 24 90 01 	vmovaps %xmm3,0x190(%rsp)
     c80:	00 00 
     c82:	c5 78 29 ce          	vmovaps %xmm9,%xmm6
     c86:	c5 7a 5c f3          	vsubss %xmm3,%xmm0,%xmm14
     c8a:	c4 41 0a 59 f6       	vmulss %xmm14,%xmm14,%xmm14
     c8f:	c5 8a 58 e4          	vaddss %xmm4,%xmm14,%xmm4
     c93:	c5 22 59 f4          	vmulss %xmm4,%xmm11,%xmm14
     c97:	c5 78 29 e3          	vmovaps %xmm12,%xmm3
     c9b:	c4 41 0a 58 f4       	vaddss %xmm12,%xmm14,%xmm14
     ca0:	c4 41 5a 5e e6       	vdivss %xmm14,%xmm4,%xmm12
     ca5:	c4 41 1a 58 e6       	vaddss %xmm14,%xmm12,%xmm12
     caa:	c4 41 1a 59 e7       	vmulss %xmm15,%xmm12,%xmm12
     caf:	c4 41 5a 5e f4       	vdivss %xmm12,%xmm4,%xmm14
     cb4:	c4 41 0a 58 e4       	vaddss %xmm12,%xmm14,%xmm12
     cb9:	c4 41 1a 59 e7       	vmulss %xmm15,%xmm12,%xmm12
     cbe:	c4 41 5a 5e f4       	vdivss %xmm12,%xmm4,%xmm14
     cc3:	c4 41 0a 58 e4       	vaddss %xmm12,%xmm14,%xmm12
     cc8:	c4 41 1a 59 e7       	vmulss %xmm15,%xmm12,%xmm12
     ccd:	c4 c1 5a 5e e4       	vdivss %xmm12,%xmm4,%xmm4
     cd2:	c5 9a 58 e4          	vaddss %xmm4,%xmm12,%xmm4
     cd6:	c5 fa 11 a4 24 b0 00 	vmovss %xmm4,0xb0(%rsp)
     cdd:	00 00 
     cdf:	c5 51 c6 dd 01       	vshufpd $0x1,%xmm5,%xmm5,%xmm11
     ce4:	c4 c1 3a 5c cb       	vsubss %xmm11,%xmm8,%xmm1
     ce9:	c5 7a 10 44 24 08    	vmovss 0x8(%rsp),%xmm8
     cef:	c4 c1 6a 5c d0       	vsubss %xmm8,%xmm2,%xmm2
     cf4:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
     cf8:	c5 ea 59 d2          	vmulss %xmm2,%xmm2,%xmm2
     cfc:	c5 f2 58 ca          	vaddss %xmm2,%xmm1,%xmm1
     d00:	c5 50 c6 cd ff       	vshufps $0xff,%xmm5,%xmm5,%xmm9
     d05:	c4 c1 7a 5c c1       	vsubss %xmm9,%xmm0,%xmm0
     d0a:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
     d0e:	c5 fa 58 c1          	vaddss %xmm1,%xmm0,%xmm0
     d12:	c5 7a 10 35 00 00 00 	vmovss 0x0(%rip),%xmm14        # d1a <simulate+0xd1a>
     d19:	00 
     d1a:	c5 8a 59 c8          	vmulss %xmm0,%xmm14,%xmm1
     d1e:	c5 f2 58 cb          	vaddss %xmm3,%xmm1,%xmm1
     d22:	c5 7a 5e e1          	vdivss %xmm1,%xmm0,%xmm12
     d26:	c5 9a 58 c9          	vaddss %xmm1,%xmm12,%xmm1
     d2a:	c5 82 59 c9          	vmulss %xmm1,%xmm15,%xmm1
     d2e:	c5 7a 5e e1          	vdivss %xmm1,%xmm0,%xmm12
     d32:	c5 9a 58 c9          	vaddss %xmm1,%xmm12,%xmm1
     d36:	c5 82 59 c9          	vmulss %xmm1,%xmm15,%xmm1
     d3a:	c5 7a 5e e1          	vdivss %xmm1,%xmm0,%xmm12
     d3e:	c5 9a 58 c9          	vaddss %xmm1,%xmm12,%xmm1
     d42:	c5 82 59 c9          	vmulss %xmm1,%xmm15,%xmm1
     d46:	c5 fa 5e c1          	vdivss %xmm1,%xmm0,%xmm0
     d4a:	c5 fa 58 c1          	vaddss %xmm1,%xmm0,%xmm0
     d4e:	c5 fa 11 84 24 a0 00 	vmovss %xmm0,0xa0(%rsp)
     d55:	00 00 
     d57:	c5 f8 28 54 24 20    	vmovaps 0x20(%rsp),%xmm2
     d5d:	c5 ea 5c c7          	vsubss %xmm7,%xmm2,%xmm0
     d61:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
     d65:	c4 c1 12 5c ca       	vsubss %xmm10,%xmm13,%xmm1
     d6a:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
     d6e:	c5 f2 58 c0          	vaddss %xmm0,%xmm1,%xmm0
     d72:	c5 fa 10 bc 24 f0 00 	vmovss 0xf0(%rsp),%xmm7
     d79:	00 00 
     d7b:	c5 fa 10 a4 24 e0 00 	vmovss 0xe0(%rsp),%xmm4
     d82:	00 00 
     d84:	c5 c2 5c cc          	vsubss %xmm4,%xmm7,%xmm1
     d88:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
     d8c:	c5 fa 58 c1          	vaddss %xmm1,%xmm0,%xmm0
     d90:	c5 8a 59 c8          	vmulss %xmm0,%xmm14,%xmm1
     d94:	c5 f2 58 cb          	vaddss %xmm3,%xmm1,%xmm1
     d98:	c5 78 28 d3          	vmovaps %xmm3,%xmm10
     d9c:	c5 7a 5e e1          	vdivss %xmm1,%xmm0,%xmm12
     da0:	c5 9a 58 c9          	vaddss %xmm1,%xmm12,%xmm1
     da4:	c5 82 59 c9          	vmulss %xmm1,%xmm15,%xmm1
     da8:	c5 7a 5e e1          	vdivss %xmm1,%xmm0,%xmm12
     dac:	c5 9a 58 c9          	vaddss %xmm1,%xmm12,%xmm1
     db0:	c5 82 59 c9          	vmulss %xmm1,%xmm15,%xmm1
     db4:	c5 7a 5e e1          	vdivss %xmm1,%xmm0,%xmm12
     db8:	c5 9a 58 c9          	vaddss %xmm1,%xmm12,%xmm1
     dbc:	c5 82 59 c9          	vmulss %xmm1,%xmm15,%xmm1
     dc0:	c5 fa 5e c1          	vdivss %xmm1,%xmm0,%xmm0
     dc4:	c5 fa 58 c1          	vaddss %xmm1,%xmm0,%xmm0
     dc8:	c5 fa 11 84 24 90 00 	vmovss %xmm0,0x90(%rsp)
     dcf:	00 00 
     dd1:	c5 f8 28 de          	vmovaps %xmm6,%xmm3
     dd5:	c5 ea 5c c6          	vsubss %xmm6,%xmm2,%xmm0
     dd9:	c5 f8 28 ea          	vmovaps %xmm2,%xmm5
     ddd:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
     de1:	c5 fa 10 b4 24 d0 00 	vmovss 0xd0(%rsp),%xmm6
     de8:	00 00 
     dea:	c5 c2 5c ce          	vsubss %xmm6,%xmm7,%xmm1
     dee:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
     df2:	c5 fa 58 c1          	vaddss %xmm1,%xmm0,%xmm0
     df6:	c5 f8 28 94 24 90 01 	vmovaps 0x190(%rsp),%xmm2
     dfd:	00 00 
     dff:	c5 92 5c ca          	vsubss %xmm2,%xmm13,%xmm1
     e03:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
     e07:	c5 f2 58 c0          	vaddss %xmm0,%xmm1,%xmm0
     e0b:	c5 8a 59 c8          	vmulss %xmm0,%xmm14,%xmm1
     e0f:	c5 aa 58 c9          	vaddss %xmm1,%xmm10,%xmm1
     e13:	c5 7a 5e e1          	vdivss %xmm1,%xmm0,%xmm12
     e17:	c5 9a 58 c9          	vaddss %xmm1,%xmm12,%xmm1
     e1b:	c5 82 59 c9          	vmulss %xmm1,%xmm15,%xmm1
     e1f:	c5 7a 5e e1          	vdivss %xmm1,%xmm0,%xmm12
     e23:	c5 9a 58 c9          	vaddss %xmm1,%xmm12,%xmm1
     e27:	c5 82 59 c9          	vmulss %xmm1,%xmm15,%xmm1
     e2b:	c5 7a 5e e1          	vdivss %xmm1,%xmm0,%xmm12
     e2f:	c5 9a 58 c9          	vaddss %xmm1,%xmm12,%xmm1
     e33:	c5 82 59 c9          	vmulss %xmm1,%xmm15,%xmm1
     e37:	c5 fa 5e c1          	vdivss %xmm1,%xmm0,%xmm0
     e3b:	c5 fa 58 c1          	vaddss %xmm1,%xmm0,%xmm0
     e3f:	c5 fa 11 84 24 60 01 	vmovss %xmm0,0x160(%rsp)
     e46:	00 00 
     e48:	c4 c1 52 5c c3       	vsubss %xmm11,%xmm5,%xmm0
     e4d:	c4 c1 12 5c e9       	vsubss %xmm9,%xmm13,%xmm5
     e52:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
     e56:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     e5a:	c5 d2 58 c0          	vaddss %xmm0,%xmm5,%xmm0
     e5e:	c4 c1 42 5c e8       	vsubss %xmm8,%xmm7,%xmm5
     e63:	c5 78 29 c1          	vmovaps %xmm8,%xmm1
     e67:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     e6b:	c5 fa 58 c5          	vaddss %xmm5,%xmm0,%xmm0
     e6f:	c5 8a 59 e8          	vmulss %xmm0,%xmm14,%xmm5
     e73:	c5 aa 58 ed          	vaddss %xmm5,%xmm10,%xmm5
     e77:	c5 fa 5e fd          	vdivss %xmm5,%xmm0,%xmm7
     e7b:	c5 c2 58 ed          	vaddss %xmm5,%xmm7,%xmm5
     e7f:	c5 82 59 ed          	vmulss %xmm5,%xmm15,%xmm5
     e83:	c5 fa 5e fd          	vdivss %xmm5,%xmm0,%xmm7
     e87:	c5 c2 58 ed          	vaddss %xmm5,%xmm7,%xmm5
     e8b:	c5 82 59 ed          	vmulss %xmm5,%xmm15,%xmm5
     e8f:	c5 fa 5e fd          	vdivss %xmm5,%xmm0,%xmm7
     e93:	c5 c2 58 ed          	vaddss %xmm5,%xmm7,%xmm5
     e97:	c5 82 59 ed          	vmulss %xmm5,%xmm15,%xmm5
     e9b:	c5 fa 5e c5          	vdivss %xmm5,%xmm0,%xmm0
     e9f:	c5 fa 58 c5          	vaddss %xmm5,%xmm0,%xmm0
     ea3:	c5 fa 11 44 24 20    	vmovss %xmm0,0x20(%rsp)
     ea9:	c5 78 28 84 24 80 01 	vmovaps 0x180(%rsp),%xmm8
     eb0:	00 00 
     eb2:	c5 ba 5c eb          	vsubss %xmm3,%xmm8,%xmm5
     eb6:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     eba:	c5 f8 28 c4          	vmovaps %xmm4,%xmm0
     ebe:	c5 da 5c fe          	vsubss %xmm6,%xmm4,%xmm7
     ec2:	c5 c2 59 ff          	vmulss %xmm7,%xmm7,%xmm7
     ec6:	c5 d2 58 ef          	vaddss %xmm7,%xmm5,%xmm5
     eca:	c5 f8 28 a4 24 70 01 	vmovaps 0x170(%rsp),%xmm4
     ed1:	00 00 
     ed3:	c5 da 5c fa          	vsubss %xmm2,%xmm4,%xmm7
     ed7:	c5 c2 59 ff          	vmulss %xmm7,%xmm7,%xmm7
     edb:	c5 c2 58 ed          	vaddss %xmm5,%xmm7,%xmm5
     edf:	c5 8a 59 fd          	vmulss %xmm5,%xmm14,%xmm7
     ee3:	c5 aa 58 ff          	vaddss %xmm7,%xmm10,%xmm7
     ee7:	c4 41 78 28 ea       	vmovaps %xmm10,%xmm13
     eec:	c5 52 5e e7          	vdivss %xmm7,%xmm5,%xmm12
     ef0:	c5 9a 58 ff          	vaddss %xmm7,%xmm12,%xmm7
     ef4:	c5 82 59 ff          	vmulss %xmm7,%xmm15,%xmm7
     ef8:	c5 52 5e e7          	vdivss %xmm7,%xmm5,%xmm12
     efc:	c5 9a 58 ff          	vaddss %xmm7,%xmm12,%xmm7
     f00:	c5 82 59 ff          	vmulss %xmm7,%xmm15,%xmm7
     f04:	c5 52 5e e7          	vdivss %xmm7,%xmm5,%xmm12
     f08:	c5 9a 58 ff          	vaddss %xmm7,%xmm12,%xmm7
     f0c:	c5 82 59 ff          	vmulss %xmm7,%xmm15,%xmm7
     f10:	c5 d2 5e ef          	vdivss %xmm7,%xmm5,%xmm5
     f14:	c5 d2 58 ef          	vaddss %xmm7,%xmm5,%xmm5
     f18:	c4 c1 3a 5c fb       	vsubss %xmm11,%xmm8,%xmm7
     f1d:	c4 41 5a 5c c1       	vsubss %xmm9,%xmm4,%xmm8
     f22:	c5 c2 59 ff          	vmulss %xmm7,%xmm7,%xmm7
     f26:	c4 41 3a 59 c0       	vmulss %xmm8,%xmm8,%xmm8
     f2b:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
     f2f:	c5 7a 5c c1          	vsubss %xmm1,%xmm0,%xmm8
     f33:	c4 41 3a 59 c0       	vmulss %xmm8,%xmm8,%xmm8
     f38:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
     f3c:	c5 0a 59 c7          	vmulss %xmm7,%xmm14,%xmm8
     f40:	c4 41 3a 58 c2       	vaddss %xmm10,%xmm8,%xmm8
     f45:	c4 41 42 5e d0       	vdivss %xmm8,%xmm7,%xmm10
     f4a:	c4 41 2a 58 c0       	vaddss %xmm8,%xmm10,%xmm8
     f4f:	c4 41 3a 59 c7       	vmulss %xmm15,%xmm8,%xmm8
     f54:	c4 41 42 5e d0       	vdivss %xmm8,%xmm7,%xmm10
     f59:	c4 41 2a 58 c0       	vaddss %xmm8,%xmm10,%xmm8
     f5e:	c4 41 3a 59 c7       	vmulss %xmm15,%xmm8,%xmm8
     f63:	c4 41 42 5e d0       	vdivss %xmm8,%xmm7,%xmm10
     f68:	c4 41 2a 58 c0       	vaddss %xmm8,%xmm10,%xmm8
     f6d:	c4 41 3a 59 c7       	vmulss %xmm15,%xmm8,%xmm8
     f72:	c4 c1 42 5e f8       	vdivss %xmm8,%xmm7,%xmm7
     f77:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
     f7b:	c4 c1 62 5c e3       	vsubss %xmm11,%xmm3,%xmm4
     f80:	c4 c1 6a 5c d1       	vsubss %xmm9,%xmm2,%xmm2
     f85:	c5 fa 11 4f 78       	vmovss %xmm1,0x78(%rdi)
     f8a:	c5 ca 5c f1          	vsubss %xmm1,%xmm6,%xmm6
     f8e:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
     f92:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
     f96:	c5 da 58 e6          	vaddss %xmm6,%xmm4,%xmm4
     f9a:	c5 ea 59 d2          	vmulss %xmm2,%xmm2,%xmm2
     f9e:	c5 ea 58 d4          	vaddss %xmm4,%xmm2,%xmm2
     fa2:	c5 8a 59 e2          	vmulss %xmm2,%xmm14,%xmm4
     fa6:	c5 92 58 e4          	vaddss %xmm4,%xmm13,%xmm4
     faa:	c5 ea 5e f4          	vdivss %xmm4,%xmm2,%xmm6
     fae:	c5 ca 58 e4          	vaddss %xmm4,%xmm6,%xmm4
     fb2:	c5 82 59 e4          	vmulss %xmm4,%xmm15,%xmm4
     fb6:	c5 ea 5e f4          	vdivss %xmm4,%xmm2,%xmm6
     fba:	c5 ca 58 e4          	vaddss %xmm4,%xmm6,%xmm4
     fbe:	c5 82 59 e4          	vmulss %xmm4,%xmm15,%xmm4
     fc2:	c5 ea 5e f4          	vdivss %xmm4,%xmm2,%xmm6
     fc6:	c5 ca 58 e4          	vaddss %xmm4,%xmm6,%xmm4
     fca:	c5 82 59 dc          	vmulss %xmm4,%xmm15,%xmm3
     fce:	c5 ea 5e d3          	vdivss %xmm3,%xmm2,%xmm2
     fd2:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
     fd6:	c5 f8 28 5c 24 40    	vmovaps 0x40(%rsp),%xmm3
     fdc:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     fe0:	c5 f8 28 a4 24 20 01 	vmovaps 0x120(%rsp),%xmm4
     fe7:	00 00 
     fe9:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
     fed:	c5 da 58 db          	vaddss %xmm3,%xmm4,%xmm3
     ff1:	c5 f8 28 a4 24 50 01 	vmovaps 0x150(%rsp),%xmm4
     ff8:	00 00 
     ffa:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
     ffe:	c5 e2 58 dc          	vaddss %xmm4,%xmm3,%xmm3
    1002:	c5 f8 28 a4 24 40 01 	vmovaps 0x140(%rsp),%xmm4
    1009:	00 00 
    100b:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
    100f:	c5 f8 28 b4 24 30 01 	vmovaps 0x130(%rsp),%xmm6
    1016:	00 00 
    1018:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
    101c:	c5 ca 58 e4          	vaddss %xmm4,%xmm6,%xmm4
    1020:	c5 fa 10 74 24 04    	vmovss 0x4(%rsp),%xmm6
    1026:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
    102a:	c5 da 58 e6          	vaddss %xmm6,%xmm4,%xmm4
    102e:	c5 f8 28 b4 24 00 01 	vmovaps 0x100(%rsp),%xmm6
    1035:	00 00 
    1037:	c5 c8 59 f6          	vmulps %xmm6,%xmm6,%xmm6
    103b:	c5 7a 16 c6          	vmovshdup %xmm6,%xmm8
    103f:	c5 ba 58 f6          	vaddss %xmm6,%xmm8,%xmm6
    1043:	c5 7a 10 84 24 10 01 	vmovss 0x110(%rsp),%xmm8
    104a:	00 00 
    104c:	c4 41 3a 59 c0       	vmulss %xmm8,%xmm8,%xmm8
    1051:	c5 ba 58 f6          	vaddss %xmm6,%xmm8,%xmm6
    1055:	c5 da 59 25 00 00 00 	vmulss 0x0(%rip),%xmm4,%xmm4        # 105d <simulate+0x105d>
    105c:	00 
    105d:	c5 ca 59 35 00 00 00 	vmulss 0x0(%rip),%xmm6,%xmm6        # 1065 <simulate+0x1065>
    1064:	00 
    1065:	c5 da 58 e6          	vaddss %xmm6,%xmm4,%xmm4
    1069:	c5 f8 28 74 24 30    	vmovaps 0x30(%rsp),%xmm6
    106f:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
    1073:	c5 78 28 44 24 60    	vmovaps 0x60(%rsp),%xmm8
    1079:	c4 41 3a 59 c0       	vmulss %xmm8,%xmm8,%xmm8
    107e:	c5 ba 58 f6          	vaddss %xmm6,%xmm8,%xmm6
    1082:	c5 78 28 44 24 10    	vmovaps 0x10(%rsp),%xmm8
    1088:	c4 41 3a 59 c0       	vmulss %xmm8,%xmm8,%xmm8
    108d:	c5 ba 58 f6          	vaddss %xmm6,%xmm8,%xmm6
    1091:	c5 ca 59 35 00 00 00 	vmulss 0x0(%rip),%xmm6,%xmm6        # 1099 <simulate+0x1099>
    1098:	00 
    1099:	c5 da 58 e6          	vaddss %xmm6,%xmm4,%xmm4
    109d:	c5 f8 28 b4 24 80 00 	vmovaps 0x80(%rsp),%xmm6
    10a4:	00 00 
    10a6:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
    10aa:	c5 78 28 44 24 70    	vmovaps 0x70(%rsp),%xmm8
    10b0:	c4 41 3a 59 c0       	vmulss %xmm8,%xmm8,%xmm8
    10b5:	c5 ba 58 f6          	vaddss %xmm6,%xmm8,%xmm6
    10b9:	c5 7a 10 44 24 0c    	vmovss 0xc(%rsp),%xmm8
    10bf:	c4 41 3a 59 c0       	vmulss %xmm8,%xmm8,%xmm8
    10c4:	c5 ba 58 f6          	vaddss %xmm6,%xmm8,%xmm6
    10c8:	c5 ca 59 35 00 00 00 	vmulss 0x0(%rip),%xmm6,%xmm6        # 10d0 <simulate+0x10d0>
    10cf:	00 
    10d0:	c5 da 58 e6          	vaddss %xmm6,%xmm4,%xmm4
    10d4:	c5 e2 59 1d 00 00 00 	vmulss 0x0(%rip),%xmm3,%xmm3        # 10dc <simulate+0x10dc>
    10db:	00 
    10dc:	c5 da 58 db          	vaddss %xmm3,%xmm4,%xmm3
    10e0:	c5 fa 10 25 00 00 00 	vmovss 0x0(%rip),%xmm4        # 10e8 <simulate+0x10e8>
    10e7:	00 
    10e8:	c5 da 5e d2          	vdivss %xmm2,%xmm4,%xmm2
    10ec:	c5 fa 10 25 00 00 00 	vmovss 0x0(%rip),%xmm4        # 10f4 <simulate+0x10f4>
    10f3:	00 
    10f4:	c5 da 5e e7          	vdivss %xmm7,%xmm4,%xmm4
    10f8:	c5 da 58 d2          	vaddss %xmm2,%xmm4,%xmm2
    10fc:	c5 fa 10 25 00 00 00 	vmovss 0x0(%rip),%xmm4        # 1104 <simulate+0x1104>
    1103:	00 
    1104:	c5 da 5e e5          	vdivss %xmm5,%xmm4,%xmm4
    1108:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # 1110 <simulate+0x1110>
    110f:	00 
    1110:	c5 d2 5e 44 24 20    	vdivss 0x20(%rsp),%xmm5,%xmm0
    1116:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # 111e <simulate+0x111e>
    111d:	00 
    111e:	c5 d2 5e 8c 24 60 01 	vdivss 0x160(%rsp),%xmm5,%xmm1
    1125:	00 00 
    1127:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # 112f <simulate+0x112f>
    112e:	00 
    112f:	c5 d2 5e ac 24 90 00 	vdivss 0x90(%rsp),%xmm5,%xmm5
    1136:	00 00 
    1138:	c5 d2 58 c9          	vaddss %xmm1,%xmm5,%xmm1
    113c:	c5 f2 58 cc          	vaddss %xmm4,%xmm1,%xmm1
    1140:	c5 fa 10 25 00 00 00 	vmovss 0x0(%rip),%xmm4        # 1148 <simulate+0x1148>
    1147:	00 
    1148:	c5 da 5e a4 24 b0 00 	vdivss 0xb0(%rsp),%xmm4,%xmm4
    114f:	00 00 
    1151:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # 1159 <simulate+0x1159>
    1158:	00 
    1159:	c5 d2 5e 6c 24 50    	vdivss 0x50(%rsp),%xmm5,%xmm5
    115f:	c5 d2 58 e4          	vaddss %xmm4,%xmm5,%xmm4
    1163:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # 116b <simulate+0x116b>
    116a:	00 
    116b:	c5 d2 5e ac 24 c0 00 	vdivss 0xc0(%rsp),%xmm5,%xmm5
    1172:	00 00 
    1174:	c5 e2 58 dd          	vaddss %xmm5,%xmm3,%xmm3
    1178:	c5 e2 58 dc          	vaddss %xmm4,%xmm3,%xmm3
    117c:	c5 fa 10 25 00 00 00 	vmovss 0x0(%rip),%xmm4        # 1184 <simulate+0x1184>
    1183:	00 
    1184:	c5 da 5e a4 24 a0 00 	vdivss 0xa0(%rsp),%xmm4,%xmm4
    118b:	00 00 
    118d:	c5 f2 58 cc          	vaddss %xmm4,%xmm1,%xmm1
    1191:	c5 f2 58 c0          	vaddss %xmm0,%xmm1,%xmm0
    1195:	c5 e2 58 c0          	vaddss %xmm0,%xmm3,%xmm0
    1199:	c5 fa 58 c2          	vaddss %xmm2,%xmm0,%xmm0
    119d:	48 ff c0             	inc    %rax
    11a0:	48 89 47 08          	mov    %rax,0x8(%rdi)
    11a4:	48 b9 a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%rcx
    11ab:	24 03 e5 
    11ae:	48 0f af c8          	imul   %rax,%rcx
    11b2:	48 b8 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%rax
    11b9:	6b 00 00 
    11bc:	48 01 c8             	add    %rcx,%rax
    11bf:	48 0f ac c0 06       	shrd   $0x6,%rax,%rax
    11c4:	48 b9 94 57 53 fe 5a 	movabs $0x35afe535794,%rcx
    11cb:	03 00 00 
    11ce:	48 39 c8             	cmp    %rcx,%rax
    11d1:	76 16                	jbe    11e9 <simulate+0x11e9>
    11d3:	48 8b 43 08          	mov    0x8(%rbx),%rax
    11d7:	48 3b 03             	cmp    (%rbx),%rax
    11da:	75 2d                	jne    1209 <simulate+0x1209>
    11dc:	48 81 c4 a0 01 00 00 	add    $0x1a0,%rsp
    11e3:	5b                   	pop    %rbx
    11e4:	e9 00 00 00 00       	jmp    11e9 <simulate+0x11e9>
    11e9:	c5 fa 11 44 24 10    	vmovss %xmm0,0x10(%rsp)
    11ef:	c5 fa 10 44 24 10    	vmovss 0x10(%rsp),%xmm0
    11f5:	e8 00 00 00 00       	call   11fa <simulate+0x11fa>
    11fa:	c5 fa 10 44 24 10    	vmovss 0x10(%rsp),%xmm0
    1200:	48 8b 43 08          	mov    0x8(%rbx),%rax
    1204:	48 3b 03             	cmp    (%rbx),%rax
    1207:	74 d3                	je     11dc <simulate+0x11dc>
    1209:	48 81 c4 a0 01 00 00 	add    $0x1a0,%rsp
    1210:	5b                   	pop    %rbx
    1211:	c3                   	ret
    1212:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1219:	1f 84 00 00 00 00 00 

0000000000001220 <init_state>:
    1220:	53                   	push   %rbx
    1221:	48 89 fb             	mov    %rdi,%rbx
    1224:	bf 00 00 00 00       	mov    $0x0,%edi
    1229:	e8 00 00 00 00       	call   122e <init_state+0xe>
    122e:	48 89 03             	mov    %rax,(%rbx)
    1231:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    1235:	c5 f8 11 43 18       	vmovups %xmm0,0x18(%rbx)
    123a:	c5 f8 11 43 08       	vmovups %xmm0,0x8(%rbx)
    123f:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # 1247 <init_state+0x27>
    1246:	00 
    1247:	c5 f8 11 43 28       	vmovups %xmm0,0x28(%rbx)
    124c:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # 1254 <init_state+0x34>
    1253:	00 
    1254:	c5 f8 11 43 38       	vmovups %xmm0,0x38(%rbx)
    1259:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # 1261 <init_state+0x41>
    1260:	00 
    1261:	c5 f8 11 43 48       	vmovups %xmm0,0x48(%rbx)
    1266:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # 126e <init_state+0x4e>
    126d:	00 
    126e:	c5 f8 11 43 58       	vmovups %xmm0,0x58(%rbx)
    1273:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # 127b <init_state+0x5b>
    127a:	00 
    127b:	c5 f8 11 43 68       	vmovups %xmm0,0x68(%rbx)
    1280:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # 1288 <init_state+0x68>
    1287:	00 
    1288:	c5 f8 11 43 78       	vmovups %xmm0,0x78(%rbx)
    128d:	5b                   	pop    %rbx
    128e:	c3                   	ret
    128f:	90                   	nop

0000000000001290 <main>:
    1290:	41 57                	push   %r15
    1292:	41 56                	push   %r14
    1294:	41 55                	push   %r13
    1296:	41 54                	push   %r12
    1298:	53                   	push   %rbx
    1299:	48 81 ec 80 01 00 00 	sub    $0x180,%rsp
    12a0:	bf 00 00 00 00       	mov    $0x0,%edi
    12a5:	e8 00 00 00 00       	call   12aa <main+0x1a>
    12aa:	48 89 c3             	mov    %rax,%rbx
    12ad:	c5 fc 28 05 00 00 00 	vmovaps 0x0(%rip),%ymm0        # 12b5 <main+0x25>
    12b4:	00 
    12b5:	c5 fc 11 84 24 40 01 	vmovups %ymm0,0x140(%rsp)
    12bc:	00 00 
    12be:	c5 fc 28 05 00 00 00 	vmovaps 0x0(%rip),%ymm0        # 12c6 <main+0x36>
    12c5:	00 
    12c6:	c5 fc 11 84 24 60 01 	vmovups %ymm0,0x160(%rsp)
    12cd:	00 00 
    12cf:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # 12d7 <main+0x47>
    12d6:	00 
    12d7:	c5 f8 29 84 24 90 00 	vmovaps %xmm0,0x90(%rsp)
    12de:	00 00 
    12e0:	c5 7b 12 0d 00 00 00 	vmovddup 0x0(%rip),%xmm9        # 12e8 <main+0x58>
    12e7:	00 
    12e8:	c5 fb 12 05 00 00 00 	vmovddup 0x0(%rip),%xmm0        # 12f0 <main+0x60>
    12ef:	00 
    12f0:	c5 fb 12 25 00 00 00 	vmovddup 0x0(%rip),%xmm4        # 12f8 <main+0x68>
    12f7:	00 
    12f8:	c5 78 28 2d 00 00 00 	vmovaps 0x0(%rip),%xmm13        # 1300 <main+0x70>
    12ff:	00 
    1300:	45 31 f6             	xor    %r14d,%r14d
    1303:	49 bf a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%r15
    130a:	24 03 e5 
    130d:	49 bc 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%r12
    1314:	6b 00 00 
    1317:	49 bd 94 57 53 fe 5a 	movabs $0x35afe535794,%r13
    131e:	03 00 00 
    1321:	eb 16                	jmp    1339 <main+0xa9>
    1323:	66 66 66 66 2e 0f 1f 	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    132a:	84 00 00 00 00 00 
    1330:	49 39 de             	cmp    %rbx,%r14
    1333:	0f 84 2a 09 00 00    	je     1c63 <main+0x9d3>
    1339:	49 39 de             	cmp    %rbx,%r14
    133c:	7d f2                	jge    1330 <main+0xa0>
    133e:	c5 f8 29 84 24 a0 00 	vmovaps %xmm0,0xa0(%rsp)
    1345:	00 00 
    1347:	c5 7c 10 bc 24 60 01 	vmovups 0x160(%rsp),%ymm15
    134e:	00 00 
    1350:	c4 c1 00 c6 c7 ff    	vshufps $0xff,%xmm15,%xmm15,%xmm0
    1356:	c5 f8 29 44 24 10    	vmovaps %xmm0,0x10(%rsp)
    135c:	c5 92 5c c0          	vsubss %xmm0,%xmm13,%xmm0
    1360:	c4 c1 01 c6 cf 01    	vshufpd $0x1,%xmm15,%xmm15,%xmm1
    1366:	c5 f9 29 4c 24 50    	vmovapd %xmm1,0x50(%rsp)
    136c:	c4 c1 7a 16 d5       	vmovshdup %xmm13,%xmm2
    1371:	c5 ea 5c c9          	vsubss %xmm1,%xmm2,%xmm1
    1375:	c4 41 7a 16 c7       	vmovshdup %xmm15,%xmm8
    137a:	c5 f8 28 ac 24 90 00 	vmovaps 0x90(%rsp),%xmm5
    1381:	00 00 
    1383:	c4 c1 52 5c d8       	vsubss %xmm8,%xmm5,%xmm3
    1388:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
    138c:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
    1390:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
    1394:	c5 f2 58 cb          	vaddss %xmm3,%xmm1,%xmm1
    1398:	c5 f2 58 c0          	vaddss %xmm0,%xmm1,%xmm0
    139c:	c5 fa 10 1d 00 00 00 	vmovss 0x0(%rip),%xmm3        # 13a4 <main+0x114>
    13a3:	00 
    13a4:	c5 fa 59 cb          	vmulss %xmm3,%xmm0,%xmm1
    13a8:	c5 78 28 db          	vmovaps %xmm3,%xmm11
    13ac:	c5 7a 10 15 00 00 00 	vmovss 0x0(%rip),%xmm10        # 13b4 <main+0x124>
    13b3:	00 
    13b4:	c5 aa 58 c9          	vaddss %xmm1,%xmm10,%xmm1
    13b8:	c5 fa 5e d9          	vdivss %xmm1,%xmm0,%xmm3
    13bc:	c5 e2 58 c9          	vaddss %xmm1,%xmm3,%xmm1
    13c0:	c5 7a 10 35 00 00 00 	vmovss 0x0(%rip),%xmm14        # 13c8 <main+0x138>
    13c7:	00 
    13c8:	c5 8a 59 c9          	vmulss %xmm1,%xmm14,%xmm1
    13cc:	c5 fa 5e d9          	vdivss %xmm1,%xmm0,%xmm3
    13d0:	c5 e2 58 c9          	vaddss %xmm1,%xmm3,%xmm1
    13d4:	c5 8a 59 c9          	vmulss %xmm1,%xmm14,%xmm1
    13d8:	c5 fa 5e d9          	vdivss %xmm1,%xmm0,%xmm3
    13dc:	c5 e2 58 c9          	vaddss %xmm1,%xmm3,%xmm1
    13e0:	c5 8a 59 c9          	vmulss %xmm1,%xmm14,%xmm1
    13e4:	c5 fa 5e c1          	vdivss %xmm1,%xmm0,%xmm0
    13e8:	c5 fa 58 c1          	vaddss %xmm1,%xmm0,%xmm0
    13ec:	c5 fa 11 44 24 08    	vmovss %xmm0,0x8(%rsp)
    13f2:	c4 c1 12 5c cf       	vsubss %xmm15,%xmm13,%xmm1
    13f7:	c4 c1 7a 16 f9       	vmovshdup %xmm9,%xmm7
    13fc:	c5 ea 5c df          	vsubss %xmm7,%xmm2,%xmm3
    1400:	c5 f8 29 7c 24 40    	vmovaps %xmm7,0x40(%rsp)
    1406:	c4 c1 52 5c f1       	vsubss %xmm9,%xmm5,%xmm6
    140b:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
    140f:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
    1413:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
    1417:	c5 ca 58 c9          	vaddss %xmm1,%xmm6,%xmm1
    141b:	c5 e2 58 c9          	vaddss %xmm1,%xmm3,%xmm1
    141f:	c5 a2 59 d9          	vmulss %xmm1,%xmm11,%xmm3
    1423:	c5 aa 58 db          	vaddss %xmm3,%xmm10,%xmm3
    1427:	c5 f2 5e f3          	vdivss %xmm3,%xmm1,%xmm6
    142b:	c5 ca 58 db          	vaddss %xmm3,%xmm6,%xmm3
    142f:	c5 8a 59 db          	vmulss %xmm3,%xmm14,%xmm3
    1433:	c5 f2 5e f3          	vdivss %xmm3,%xmm1,%xmm6
    1437:	c5 ca 58 db          	vaddss %xmm3,%xmm6,%xmm3
    143b:	c5 8a 59 db          	vmulss %xmm3,%xmm14,%xmm3
    143f:	c5 f2 5e f3          	vdivss %xmm3,%xmm1,%xmm6
    1443:	c5 ca 58 db          	vaddss %xmm3,%xmm6,%xmm3
    1447:	c5 8a 59 db          	vmulss %xmm3,%xmm14,%xmm3
    144b:	c5 f2 5e cb          	vdivss %xmm3,%xmm1,%xmm1
    144f:	c5 f2 58 cb          	vaddss %xmm3,%xmm1,%xmm1
    1453:	c5 fa 11 8c 24 80 00 	vmovss %xmm1,0x80(%rsp)
    145a:	00 00 
    145c:	c5 d0 c6 c5 ff       	vshufps $0xff,%xmm5,%xmm5,%xmm0
    1461:	c5 f8 29 44 24 30    	vmovaps %xmm0,0x30(%rsp)
    1467:	c5 92 5c d8          	vsubss %xmm0,%xmm13,%xmm3
    146b:	c4 c1 10 c6 c5 ff    	vshufps $0xff,%xmm13,%xmm13,%xmm0
    1471:	c5 f8 29 44 24 20    	vmovaps %xmm0,0x20(%rsp)
    1477:	c5 ea 5c d0          	vsubss %xmm0,%xmm2,%xmm2
    147b:	c4 41 11 c6 dd 01    	vshufpd $0x1,%xmm13,%xmm13,%xmm11
    1481:	c4 c1 52 5c f3       	vsubss %xmm11,%xmm5,%xmm6
    1486:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
    148a:	c5 ea 59 d2          	vmulss %xmm2,%xmm2,%xmm2
    148e:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
    1492:	c5 ea 58 d6          	vaddss %xmm6,%xmm2,%xmm2
    1496:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
    149a:	c5 ea 59 1d 00 00 00 	vmulss 0x0(%rip),%xmm2,%xmm3        # 14a2 <main+0x212>
    14a1:	00 
    14a2:	c5 aa 58 db          	vaddss %xmm3,%xmm10,%xmm3
    14a6:	c5 ea 5e f3          	vdivss %xmm3,%xmm2,%xmm6
    14aa:	c5 ca 58 db          	vaddss %xmm3,%xmm6,%xmm3
    14ae:	c5 8a 59 db          	vmulss %xmm3,%xmm14,%xmm3
    14b2:	c5 ea 5e f3          	vdivss %xmm3,%xmm2,%xmm6
    14b6:	c5 ca 58 db          	vaddss %xmm3,%xmm6,%xmm3
    14ba:	c5 8a 59 db          	vmulss %xmm3,%xmm14,%xmm3
    14be:	c5 ea 5e f3          	vdivss %xmm3,%xmm2,%xmm6
    14c2:	c5 ca 58 db          	vaddss %xmm3,%xmm6,%xmm3
    14c6:	c5 8a 59 db          	vmulss %xmm3,%xmm14,%xmm3
    14ca:	c5 ea 5e d3          	vdivss %xmm3,%xmm2,%xmm2
    14ce:	c5 ea 58 cb          	vaddss %xmm3,%xmm2,%xmm1
    14d2:	c5 fa 11 4c 24 70    	vmovss %xmm1,0x70(%rsp)
    14d8:	c5 d0 c6 dc d6       	vshufps $0xd6,%xmm4,%xmm5,%xmm3
    14dd:	c5 e0 c6 dc 48       	vshufps $0x48,%xmm4,%xmm3,%xmm3
    14e2:	c5 78 29 ac 24 10 01 	vmovaps %xmm13,0x110(%rsp)
    14e9:	00 00 
    14eb:	c5 90 5c cb          	vsubps %xmm3,%xmm13,%xmm1
    14ef:	c5 f8 29 8c 24 00 01 	vmovaps %xmm1,0x100(%rsp)
    14f6:	00 00 
    14f8:	c5 f0 59 c1          	vmulps %xmm1,%xmm1,%xmm0
    14fc:	c5 f8 29 84 24 20 01 	vmovaps %xmm0,0x120(%rsp)
    1503:	00 00 
    1505:	c5 f8 28 4c 24 10    	vmovaps 0x10(%rsp),%xmm1
    150b:	c4 c1 72 5c df       	vsubss %xmm15,%xmm1,%xmm3
    1510:	c5 78 28 d4          	vmovaps %xmm4,%xmm10
    1514:	c5 f8 28 44 24 50    	vmovaps 0x50(%rsp),%xmm0
    151a:	c5 7a 5c e7          	vsubss %xmm7,%xmm0,%xmm12
    151e:	c4 41 3a 5c e9       	vsubss %xmm9,%xmm8,%xmm13
    1523:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
    1527:	c4 41 1a 59 e4       	vmulss %xmm12,%xmm12,%xmm12
    152c:	c4 41 12 59 ed       	vmulss %xmm13,%xmm13,%xmm13
    1531:	c4 41 1a 58 e5       	vaddss %xmm13,%xmm12,%xmm12
    1536:	c5 9a 58 db          	vaddss %xmm3,%xmm12,%xmm3
    153a:	c5 fa 10 15 00 00 00 	vmovss 0x0(%rip),%xmm2        # 1542 <main+0x2b2>
    1541:	00 
    1542:	c5 62 59 e2          	vmulss %xmm2,%xmm3,%xmm12
    1546:	c5 1a 58 25 00 00 00 	vaddss 0x0(%rip),%xmm12,%xmm12        # 154e <main+0x2be>
    154d:	00 
    154e:	c4 41 62 5e ec       	vdivss %xmm12,%xmm3,%xmm13
    1553:	c4 41 12 58 e4       	vaddss %xmm12,%xmm13,%xmm12
    1558:	c4 41 1a 59 e6       	vmulss %xmm14,%xmm12,%xmm12
    155d:	c4 41 62 5e ec       	vdivss %xmm12,%xmm3,%xmm13
    1562:	c4 41 12 58 e4       	vaddss %xmm12,%xmm13,%xmm12
    1567:	c4 41 1a 59 e6       	vmulss %xmm14,%xmm12,%xmm12
    156c:	c4 41 62 5e ec       	vdivss %xmm12,%xmm3,%xmm13
    1571:	c4 41 12 58 e4       	vaddss %xmm12,%xmm13,%xmm12
    1576:	c4 41 1a 59 e6       	vmulss %xmm14,%xmm12,%xmm12
    157b:	c4 c1 62 5e dc       	vdivss %xmm12,%xmm3,%xmm3
    1580:	c5 9a 58 db          	vaddss %xmm3,%xmm12,%xmm3
    1584:	c5 fa 11 5c 24 60    	vmovss %xmm3,0x60(%rsp)
    158a:	c5 f8 28 74 24 30    	vmovaps 0x30(%rsp),%xmm6
    1590:	c5 f2 5c e6          	vsubss %xmm6,%xmm1,%xmm4
    1594:	c5 f8 28 4c 24 20    	vmovaps 0x20(%rsp),%xmm1
    159a:	c5 fa 5c f9          	vsubss %xmm1,%xmm0,%xmm7
    159e:	c4 41 3a 5c c3       	vsubss %xmm11,%xmm8,%xmm8
    15a3:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
    15a7:	c5 c2 59 ff          	vmulss %xmm7,%xmm7,%xmm7
    15ab:	c4 41 3a 59 c0       	vmulss %xmm8,%xmm8,%xmm8
    15b0:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
    15b4:	c5 c2 58 e4          	vaddss %xmm4,%xmm7,%xmm4
    15b8:	c5 da 59 fa          	vmulss %xmm2,%xmm4,%xmm7
    15bc:	c5 78 28 ea          	vmovaps %xmm2,%xmm13
    15c0:	c5 fa 10 1d 00 00 00 	vmovss 0x0(%rip),%xmm3        # 15c8 <main+0x338>
    15c7:	00 
    15c8:	c5 c2 58 fb          	vaddss %xmm3,%xmm7,%xmm7
    15cc:	c5 5a 5e c7          	vdivss %xmm7,%xmm4,%xmm8
    15d0:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
    15d4:	c5 8a 59 ff          	vmulss %xmm7,%xmm14,%xmm7
    15d8:	c5 5a 5e c7          	vdivss %xmm7,%xmm4,%xmm8
    15dc:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
    15e0:	c5 8a 59 ff          	vmulss %xmm7,%xmm14,%xmm7
    15e4:	c5 5a 5e c7          	vdivss %xmm7,%xmm4,%xmm8
    15e8:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
    15ec:	c5 8a 59 ff          	vmulss %xmm7,%xmm14,%xmm7
    15f0:	c5 da 5e e7          	vdivss %xmm7,%xmm4,%xmm4
    15f4:	c5 da 58 d7          	vaddss %xmm7,%xmm4,%xmm2
    15f8:	c5 fa 11 54 24 50    	vmovss %xmm2,0x50(%rsp)
    15fe:	c4 c1 00 c6 ff 39    	vshufps $0x39,%xmm15,%xmm15,%xmm7
    1604:	c4 e3 41 21 fd 30    	vinsertps $0x30,%xmm5,%xmm7,%xmm7
    160a:	c4 43 51 0c c2 03    	vblendps $0x3,%xmm10,%xmm5,%xmm8
    1610:	c5 f8 28 c5          	vmovaps %xmm5,%xmm0
    1614:	c4 41 38 c6 c0 24    	vshufps $0x24,%xmm8,%xmm8,%xmm8
    161a:	c4 41 40 5c e0       	vsubps %xmm8,%xmm7,%xmm12
    161f:	c4 c1 18 59 fc       	vmulps %xmm12,%xmm12,%xmm7
    1624:	c5 c1 c6 ff 01       	vshufpd $0x1,%xmm7,%xmm7,%xmm7
    1629:	c5 f8 28 94 24 20 01 	vmovaps 0x120(%rsp),%xmm2
    1630:	00 00 
    1632:	c5 e8 58 ff          	vaddps %xmm7,%xmm2,%xmm7
    1636:	c5 fa 16 ff          	vmovshdup %xmm7,%xmm7
    163a:	c5 c2 58 ea          	vaddss %xmm2,%xmm7,%xmm5
    163e:	c5 92 59 fd          	vmulss %xmm5,%xmm13,%xmm7
    1642:	c5 c2 58 fb          	vaddss %xmm3,%xmm7,%xmm7
    1646:	c5 f8 28 e3          	vmovaps %xmm3,%xmm4
    164a:	c5 52 5e c7          	vdivss %xmm7,%xmm5,%xmm8
    164e:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
    1652:	c5 8a 59 ff          	vmulss %xmm7,%xmm14,%xmm7
    1656:	c5 52 5e c7          	vdivss %xmm7,%xmm5,%xmm8
    165a:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
    165e:	c5 8a 59 ff          	vmulss %xmm7,%xmm14,%xmm7
    1662:	c5 52 5e c7          	vdivss %xmm7,%xmm5,%xmm8
    1666:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
    166a:	c5 8a 59 ff          	vmulss %xmm7,%xmm14,%xmm7
    166e:	c5 fa 11 6c 24 0c    	vmovss %xmm5,0xc(%rsp)
    1674:	c5 52 5e c7          	vdivss %xmm7,%xmm5,%xmm8
    1678:	c5 ba 58 df          	vaddss %xmm7,%xmm8,%xmm3
    167c:	c5 fa 11 5c 24 10    	vmovss %xmm3,0x10(%rsp)
    1682:	c5 82 5c fe          	vsubss %xmm6,%xmm15,%xmm7
    1686:	c5 f8 28 5c 24 40    	vmovaps 0x40(%rsp),%xmm3
    168c:	c5 e2 5c e9          	vsubss %xmm1,%xmm3,%xmm5
    1690:	c4 41 32 5c c3       	vsubss %xmm11,%xmm9,%xmm8
    1695:	c5 c2 59 ff          	vmulss %xmm7,%xmm7,%xmm7
    1699:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
    169d:	c4 41 3a 59 c0       	vmulss %xmm8,%xmm8,%xmm8
    16a2:	c5 ba 58 ed          	vaddss %xmm5,%xmm8,%xmm5
    16a6:	c5 d2 58 ef          	vaddss %xmm7,%xmm5,%xmm5
    16aa:	c5 92 59 fd          	vmulss %xmm5,%xmm13,%xmm7
    16ae:	c5 c2 58 fc          	vaddss %xmm4,%xmm7,%xmm7
    16b2:	c5 52 5e c7          	vdivss %xmm7,%xmm5,%xmm8
    16b6:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
    16ba:	c5 8a 59 ff          	vmulss %xmm7,%xmm14,%xmm7
    16be:	c5 52 5e c7          	vdivss %xmm7,%xmm5,%xmm8
    16c2:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
    16c6:	c5 8a 59 ff          	vmulss %xmm7,%xmm14,%xmm7
    16ca:	c5 52 5e c7          	vdivss %xmm7,%xmm5,%xmm8
    16ce:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
    16d2:	c5 8a 59 ff          	vmulss %xmm7,%xmm14,%xmm7
    16d6:	c5 d2 5e ef          	vdivss %xmm7,%xmm5,%xmm5
    16da:	c5 d2 58 cf          	vaddss %xmm7,%xmm5,%xmm1
    16de:	c5 fa 11 4c 24 40    	vmovss %xmm1,0x40(%rsp)
    16e4:	c5 f8 28 e8          	vmovaps %xmm0,%xmm5
    16e8:	c4 c1 78 c6 ff 27    	vshufps $0x27,%xmm15,%xmm0,%xmm7
    16ee:	c5 78 29 4c 24 30    	vmovaps %xmm9,0x30(%rsp)
    16f4:	c4 c1 40 c6 f9 4c    	vshufps $0x4c,%xmm9,%xmm7,%xmm7
    16fa:	c5 c0 c6 ff 78       	vshufps $0x78,%xmm7,%xmm7,%xmm7
    16ff:	c5 78 29 54 24 20    	vmovaps %xmm10,0x20(%rsp)
    1705:	c4 41 78 c6 c2 4a    	vshufps $0x4a,%xmm10,%xmm0,%xmm8
    170b:	c4 41 38 c6 c0 78    	vshufps $0x78,%xmm8,%xmm8,%xmm8
    1711:	c4 c1 40 5c c8       	vsubps %xmm8,%xmm7,%xmm1
    1716:	c4 c1 19 c6 fc 01    	vshufpd $0x1,%xmm12,%xmm12,%xmm7
    171c:	c4 e3 41 21 f9 d0    	vinsertps $0xd0,%xmm1,%xmm7,%xmm7
    1722:	c5 c0 c6 ff e1       	vshufps $0xe1,%xmm7,%xmm7,%xmm7
    1727:	c5 71 c6 c1 01       	vshufpd $0x1,%xmm1,%xmm1,%xmm8
    172c:	c5 c0 59 ff          	vmulps %xmm7,%xmm7,%xmm7
    1730:	c4 43 39 0c c4 02    	vblendps $0x2,%xmm12,%xmm8,%xmm8
    1736:	c4 41 38 59 c0       	vmulps %xmm8,%xmm8,%xmm8
    173b:	c5 78 29 a4 24 f0 00 	vmovaps %xmm12,0xf0(%rsp)
    1742:	00 00 
    1744:	c4 63 19 0c c9 02    	vblendps $0x2,%xmm1,%xmm12,%xmm9
    174a:	c4 41 30 c6 c9 e1    	vshufps $0xe1,%xmm9,%xmm9,%xmm9
    1750:	c4 41 30 59 c9       	vmulps %xmm9,%xmm9,%xmm9
    1755:	c4 41 38 58 c1       	vaddps %xmm9,%xmm8,%xmm8
    175a:	c5 38 58 df          	vaddps %xmm7,%xmm8,%xmm11
    175e:	c5 a0 59 3d 00 00 00 	vmulps 0x0(%rip),%xmm11,%xmm7        # 1766 <main+0x4d6>
    1765:	00 
    1766:	c5 c0 58 3d 00 00 00 	vaddps 0x0(%rip),%xmm7,%xmm7        # 176e <main+0x4de>
    176d:	00 
    176e:	c5 78 53 c7          	vrcpps %xmm7,%xmm8
    1772:	c4 41 20 59 c8       	vmulps %xmm8,%xmm11,%xmm9
    1777:	c5 30 59 d7          	vmulps %xmm7,%xmm9,%xmm10
    177b:	c4 41 20 5c d2       	vsubps %xmm10,%xmm11,%xmm10
    1780:	c4 41 38 59 c2       	vmulps %xmm10,%xmm8,%xmm8
    1785:	c5 b0 58 ff          	vaddps %xmm7,%xmm9,%xmm7
    1789:	c5 b8 58 ff          	vaddps %xmm7,%xmm8,%xmm7
    178d:	c4 e2 79 18 05 00 00 	vbroadcastss 0x0(%rip),%xmm0        # 1796 <main+0x506>
    1794:	00 00 
    1796:	c5 c0 59 f8          	vmulps %xmm0,%xmm7,%xmm7
    179a:	c5 78 53 c7          	vrcpps %xmm7,%xmm8
    179e:	c4 41 20 59 c8       	vmulps %xmm8,%xmm11,%xmm9
    17a3:	c5 30 59 d7          	vmulps %xmm7,%xmm9,%xmm10
    17a7:	c4 41 20 5c d2       	vsubps %xmm10,%xmm11,%xmm10
    17ac:	c4 41 38 59 c2       	vmulps %xmm10,%xmm8,%xmm8
    17b1:	c5 b0 58 ff          	vaddps %xmm7,%xmm9,%xmm7
    17b5:	c5 b8 58 ff          	vaddps %xmm7,%xmm8,%xmm7
    17b9:	c5 c0 59 f8          	vmulps %xmm0,%xmm7,%xmm7
    17bd:	c5 78 53 c7          	vrcpps %xmm7,%xmm8
    17c1:	c4 41 20 59 c8       	vmulps %xmm8,%xmm11,%xmm9
    17c6:	c5 30 59 d7          	vmulps %xmm7,%xmm9,%xmm10
    17ca:	c4 41 20 5c d2       	vsubps %xmm10,%xmm11,%xmm10
    17cf:	c4 41 38 59 c2       	vmulps %xmm10,%xmm8,%xmm8
    17d4:	c5 b0 58 ff          	vaddps %xmm7,%xmm9,%xmm7
    17d8:	c5 b8 58 ff          	vaddps %xmm7,%xmm8,%xmm7
    17dc:	c5 c0 59 f8          	vmulps %xmm0,%xmm7,%xmm7
    17e0:	c5 78 53 c7          	vrcpps %xmm7,%xmm8
    17e4:	c4 41 20 59 c8       	vmulps %xmm8,%xmm11,%xmm9
    17e9:	c5 30 59 d7          	vmulps %xmm7,%xmm9,%xmm10
    17ed:	c5 78 29 9c 24 e0 00 	vmovaps %xmm11,0xe0(%rsp)
    17f4:	00 00 
    17f6:	c4 41 20 5c d2       	vsubps %xmm10,%xmm11,%xmm10
    17fb:	c4 41 38 59 c2       	vmulps %xmm10,%xmm8,%xmm8
    1800:	c5 b0 58 ff          	vaddps %xmm7,%xmm9,%xmm7
    1804:	c5 b8 58 df          	vaddps %xmm7,%xmm8,%xmm3
    1808:	c5 e9 c6 fa 01       	vshufpd $0x1,%xmm2,%xmm2,%xmm7
    180d:	c5 e8 c6 f2 5f       	vshufps $0x5f,%xmm2,%xmm2,%xmm6
    1812:	c5 ca 58 f7          	vaddss %xmm7,%xmm6,%xmm6
    1816:	c5 fc 11 8c 24 20 01 	vmovups %ymm1,0x120(%rsp)
    181d:	00 00 
    181f:	c5 f2 59 f9          	vmulss %xmm1,%xmm1,%xmm7
    1823:	c5 ca 58 f7          	vaddss %xmm7,%xmm6,%xmm6
    1827:	c5 92 59 fe          	vmulss %xmm6,%xmm13,%xmm7
    182b:	c5 c2 58 fc          	vaddss %xmm4,%xmm7,%xmm7
    182f:	c5 4a 5e c7          	vdivss %xmm7,%xmm6,%xmm8
    1833:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
    1837:	c5 8a 59 ff          	vmulss %xmm7,%xmm14,%xmm7
    183b:	c5 4a 5e c7          	vdivss %xmm7,%xmm6,%xmm8
    183f:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
    1843:	c5 8a 59 ff          	vmulss %xmm7,%xmm14,%xmm7
    1847:	c5 4a 5e c7          	vdivss %xmm7,%xmm6,%xmm8
    184b:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
    184f:	c5 8a 59 ff          	vmulss %xmm7,%xmm14,%xmm7
    1853:	c5 4a 5e c7          	vdivss %xmm7,%xmm6,%xmm8
    1857:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
    185b:	c4 63 7d 19 f9 01    	vextractf128 $0x1,%ymm15,%xmm1
    1861:	c5 78 28 bc 24 a0 00 	vmovaps 0xa0(%rsp),%xmm15
    1868:	00 00 
    186a:	c5 7a 16 c1          	vmovshdup %xmm1,%xmm8
    186e:	c4 41 3a 59 c0       	vmulss %xmm8,%xmm8,%xmm8
    1873:	c5 72 59 c9          	vmulss %xmm1,%xmm1,%xmm9
    1877:	c4 41 3a 58 c1       	vaddss %xmm9,%xmm8,%xmm8
    187c:	c5 fd 10 84 24 40 01 	vmovupd 0x140(%rsp),%ymm0
    1883:	00 00 
    1885:	c4 c3 7d 19 c3 01    	vextractf128 $0x1,%ymm0,%xmm11
    188b:	c4 c1 20 c6 e3 ff    	vshufps $0xff,%xmm11,%xmm11,%xmm4
    1891:	c5 f8 29 a4 24 c0 00 	vmovaps %xmm4,0xc0(%rsp)
    1898:	00 00 
    189a:	c5 5a 59 cc          	vmulss %xmm4,%xmm4,%xmm9
    189e:	c4 41 3a 58 c1       	vaddss %xmm9,%xmm8,%xmm8
    18a3:	c4 41 7a 16 cb       	vmovshdup %xmm11,%xmm9
    18a8:	c4 41 32 59 c9       	vmulss %xmm9,%xmm9,%xmm9
    18ad:	c4 41 22 59 d3       	vmulss %xmm11,%xmm11,%xmm10
    18b2:	c4 41 32 58 ca       	vaddss %xmm10,%xmm9,%xmm9
    18b7:	c5 78 29 9c 24 b0 00 	vmovaps %xmm11,0xb0(%rsp)
    18be:	00 00 
    18c0:	c4 41 21 c6 d3 01    	vshufpd $0x1,%xmm11,%xmm11,%xmm10
    18c6:	c4 41 2a 59 d2       	vmulss %xmm10,%xmm10,%xmm10
    18cb:	c4 41 32 58 ca       	vaddss %xmm10,%xmm9,%xmm9
    18d0:	c5 79 c6 d0 01       	vshufpd $0x1,%xmm0,%xmm0,%xmm10
    18d5:	c4 41 2a 59 d2       	vmulss %xmm10,%xmm10,%xmm10
    18da:	c5 7a 16 d8          	vmovshdup %xmm0,%xmm11
    18de:	c4 41 22 59 db       	vmulss %xmm11,%xmm11,%xmm11
    18e3:	c4 41 2a 58 d3       	vaddss %xmm11,%xmm10,%xmm10
    18e8:	c5 78 c6 d8 ff       	vshufps $0xff,%xmm0,%xmm0,%xmm11
    18ed:	c4 41 22 59 db       	vmulss %xmm11,%xmm11,%xmm11
    18f2:	c4 41 2a 58 d3       	vaddss %xmm11,%xmm10,%xmm10
    18f7:	c5 70 c6 d9 ff       	vshufps $0xff,%xmm1,%xmm1,%xmm11
    18fc:	c4 41 22 59 db       	vmulss %xmm11,%xmm11,%xmm11
    1901:	c5 f8 29 8c 24 d0 00 	vmovaps %xmm1,0xd0(%rsp)
    1908:	00 00 
    190a:	c5 71 c6 e1 01       	vshufpd $0x1,%xmm1,%xmm1,%xmm12
    190f:	c4 41 1a 59 e4       	vmulss %xmm12,%xmm12,%xmm12
    1914:	c4 41 22 58 dc       	vaddss %xmm12,%xmm11,%xmm11
    1919:	c5 7a 59 e0          	vmulss %xmm0,%xmm0,%xmm12
    191d:	c4 41 22 58 dc       	vaddss %xmm12,%xmm11,%xmm11
    1922:	c4 41 00 59 e7       	vmulps %xmm15,%xmm15,%xmm12
    1927:	c4 41 7a 16 ec       	vmovshdup %xmm12,%xmm13
    192c:	c4 41 12 58 e4       	vaddss %xmm12,%xmm13,%xmm12
    1931:	c5 50 59 ed          	vmulps %xmm5,%xmm5,%xmm13
    1935:	c4 41 7a 16 ed       	vmovshdup %xmm13,%xmm13
    193a:	c4 41 1a 58 e5       	vaddss %xmm13,%xmm12,%xmm12
    193f:	c5 2a 59 15 00 00 00 	vmulss 0x0(%rip),%xmm10,%xmm10        # 1947 <main+0x6b7>
    1946:	00 
    1947:	c5 1a 59 25 00 00 00 	vmulss 0x0(%rip),%xmm12,%xmm12        # 194f <main+0x6bf>
    194e:	00 
    194f:	c4 41 1a 58 d2       	vaddss %xmm10,%xmm12,%xmm10
    1954:	c5 22 59 1d 00 00 00 	vmulss 0x0(%rip),%xmm11,%xmm11        # 195c <main+0x6cc>
    195b:	00 
    195c:	c4 41 22 58 d2       	vaddss %xmm10,%xmm11,%xmm10
    1961:	c5 32 59 0d 00 00 00 	vmulss 0x0(%rip),%xmm9,%xmm9        # 1969 <main+0x6d9>
    1968:	00 
    1969:	c4 41 2a 58 c9       	vaddss %xmm9,%xmm10,%xmm9
    196e:	c5 3a 59 05 00 00 00 	vmulss 0x0(%rip),%xmm8,%xmm8        # 1976 <main+0x6e6>
    1975:	00 
    1976:	c4 41 32 58 c0       	vaddss %xmm8,%xmm9,%xmm8
    197b:	c5 fa 10 05 00 00 00 	vmovss 0x0(%rip),%xmm0        # 1983 <main+0x6f3>
    1982:	00 
    1983:	c5 fa 5e 54 24 70    	vdivss 0x70(%rsp),%xmm0,%xmm2
    1989:	c5 ba 58 d2          	vaddss %xmm2,%xmm8,%xmm2
    198d:	c5 fa 10 05 00 00 00 	vmovss 0x0(%rip),%xmm0        # 1995 <main+0x705>
    1994:	00 
    1995:	c5 fa 5e 8c 24 80 00 	vdivss 0x80(%rsp),%xmm0,%xmm1
    199c:	00 00 
    199e:	c5 8a 59 f6          	vmulss %xmm6,%xmm14,%xmm6
    19a2:	c5 ca 59 f7          	vmulss %xmm7,%xmm6,%xmm6
    19a6:	c5 fa 10 05 00 00 00 	vmovss 0x0(%rip),%xmm0        # 19ae <main+0x71e>
    19ad:	00 
    19ae:	c5 fa 5e ff          	vdivss %xmm7,%xmm0,%xmm7
    19b2:	c5 c2 58 c9          	vaddss %xmm1,%xmm7,%xmm1
    19b6:	c5 fa 10 7c 24 10    	vmovss 0x10(%rsp),%xmm7
    19bc:	c5 f8 28 e3          	vmovaps %xmm3,%xmm4
    19c0:	c5 fa 10 05 00 00 00 	vmovss 0x0(%rip),%xmm0        # 19c8 <main+0x738>
    19c7:	00 
    19c8:	c5 fa 5e 6c 24 40    	vdivss 0x40(%rsp),%xmm0,%xmm5
    19ce:	c5 f2 58 cd          	vaddss %xmm5,%xmm1,%xmm1
    19d2:	c5 fa 10 05 00 00 00 	vmovss 0x0(%rip),%xmm0        # 19da <main+0x74a>
    19d9:	00 
    19da:	c5 fa 5e ef          	vdivss %xmm7,%xmm0,%xmm5
    19de:	c5 ea 58 d5          	vaddss %xmm5,%xmm2,%xmm2
    19e2:	c5 ea 58 c9          	vaddss %xmm1,%xmm2,%xmm1
    19e6:	c5 fa 10 05 00 00 00 	vmovss 0x0(%rip),%xmm0        # 19ee <main+0x75e>
    19ed:	00 
    19ee:	c5 fa 5e 44 24 08    	vdivss 0x8(%rsp),%xmm0,%xmm0
    19f4:	c5 f2 58 c0          	vaddss %xmm0,%xmm1,%xmm0
    19f8:	c5 fa 10 0d 00 00 00 	vmovss 0x0(%rip),%xmm1        # 1a00 <main+0x770>
    19ff:	00 
    1a00:	c5 f2 5e 4c 24 60    	vdivss 0x60(%rsp),%xmm1,%xmm1
    1a06:	c5 fa 58 c1          	vaddss %xmm1,%xmm0,%xmm0
    1a0a:	c5 fa 10 0d 00 00 00 	vmovss 0x0(%rip),%xmm1        # 1a12 <main+0x782>
    1a11:	00 
    1a12:	c5 f2 5e 4c 24 50    	vdivss 0x50(%rsp),%xmm1,%xmm1
    1a18:	c5 fa 10 15 00 00 00 	vmovss 0x0(%rip),%xmm2        # 1a20 <main+0x790>
    1a1f:	00 
    1a20:	c5 ea 5e d6          	vdivss %xmm6,%xmm2,%xmm2
    1a24:	c5 fa 58 c1          	vaddss %xmm1,%xmm0,%xmm0
    1a28:	c5 fa 10 0d 00 00 00 	vmovss 0x0(%rip),%xmm1        # 1a30 <main+0x7a0>
    1a2f:	00 
    1a30:	c5 f2 5e cb          	vdivss %xmm3,%xmm1,%xmm1
    1a34:	c5 fa 58 c1          	vaddss %xmm1,%xmm0,%xmm0
    1a38:	c5 fa 16 cb          	vmovshdup %xmm3,%xmm1
    1a3c:	c5 fa 10 1d 00 00 00 	vmovss 0x0(%rip),%xmm3        # 1a44 <main+0x7b4>
    1a43:	00 
    1a44:	c5 e2 5e c9          	vdivss %xmm1,%xmm3,%xmm1
    1a48:	c5 fa 58 c1          	vaddss %xmm1,%xmm0,%xmm0
    1a4c:	c5 ea 59 1d 00 00 00 	vmulss 0x0(%rip),%xmm2,%xmm3        # 1a54 <main+0x7c4>
    1a53:	00 
    1a54:	c5 6a 59 05 00 00 00 	vmulss 0x0(%rip),%xmm2,%xmm8        # 1a5c <main+0x7cc>
    1a5b:	00 
    1a5c:	4c 89 f0             	mov    %r14,%rax
    1a5f:	49 0f af c7          	imul   %r15,%rax
    1a63:	4c 01 e0             	add    %r12,%rax
    1a66:	48 0f ac c0 06       	shrd   $0x6,%rax,%rax
    1a6b:	4c 39 e8             	cmp    %r13,%rax
    1a6e:	77 53                	ja     1ac3 <main+0x833>
    1a70:	c5 fa 11 44 24 08    	vmovss %xmm0,0x8(%rsp)
    1a76:	c5 fa 10 44 24 08    	vmovss 0x8(%rsp),%xmm0
    1a7c:	c5 f8 29 a4 24 80 00 	vmovaps %xmm4,0x80(%rsp)
    1a83:	00 00 
    1a85:	c5 78 29 44 24 70    	vmovaps %xmm8,0x70(%rsp)
    1a8b:	c5 f8 29 5c 24 60    	vmovaps %xmm3,0x60(%rsp)
    1a91:	c5 f8 77             	vzeroupper
    1a94:	e8 00 00 00 00       	call   1a99 <main+0x809>
    1a99:	c5 fa 10 44 24 08    	vmovss 0x8(%rsp),%xmm0
    1a9f:	c5 f8 28 5c 24 60    	vmovaps 0x60(%rsp),%xmm3
    1aa5:	c5 78 28 44 24 70    	vmovaps 0x70(%rsp),%xmm8
    1aab:	c5 f8 28 a4 24 80 00 	vmovaps 0x80(%rsp),%xmm4
    1ab2:	00 00 
    1ab4:	c5 fa 10 7c 24 10    	vmovss 0x10(%rsp),%xmm7
    1aba:	c5 78 28 bc 24 a0 00 	vmovaps 0xa0(%rsp),%xmm15
    1ac1:	00 00 
    1ac3:	49 39 de             	cmp    %rbx,%r14
    1ac6:	0f 84 8f 01 00 00    	je     1c5b <main+0x9cb>
    1acc:	c5 fa 10 44 24 0c    	vmovss 0xc(%rsp),%xmm0
    1ad2:	c5 fa 59 05 00 00 00 	vmulss 0x0(%rip),%xmm0,%xmm0        # 1ada <main+0x84a>
    1ad9:	00 
    1ada:	c5 fa 59 c7          	vmulss %xmm7,%xmm0,%xmm0
    1ade:	c5 fa 10 0d 00 00 00 	vmovss 0x0(%rip),%xmm1        # 1ae6 <main+0x856>
    1ae5:	00 
    1ae6:	c5 f2 5e c0          	vdivss %xmm0,%xmm1,%xmm0
    1aea:	c4 e3 79 21 cb 10    	vinsertps $0x10,%xmm3,%xmm0,%xmm1
    1af0:	c5 f8 28 84 24 e0 00 	vmovaps 0xe0(%rsp),%xmm0
    1af7:	00 00 
    1af9:	c5 f8 59 05 00 00 00 	vmulps 0x0(%rip),%xmm0,%xmm0        # 1b01 <main+0x871>
    1b00:	00 
    1b01:	c5 f8 59 c4          	vmulps %xmm4,%xmm0,%xmm0
    1b05:	c5 f8 53 d0          	vrcpps %xmm0,%xmm2
    1b09:	c4 e2 79 18 25 00 00 	vbroadcastss 0x0(%rip),%xmm4        # 1b12 <main+0x882>
    1b10:	00 00 
    1b12:	c5 e8 59 dc          	vmulps %xmm4,%xmm2,%xmm3
    1b16:	c5 f8 59 c3          	vmulps %xmm3,%xmm0,%xmm0
    1b1a:	c5 d8 5c c0          	vsubps %xmm0,%xmm4,%xmm0
    1b1e:	c5 e8 59 c0          	vmulps %xmm0,%xmm2,%xmm0
    1b22:	c5 e0 58 d0          	vaddps %xmm0,%xmm3,%xmm2
    1b26:	c4 c1 7a 12 c0       	vmovsldup %xmm8,%xmm0
    1b2b:	c5 f9 28 a4 24 00 01 	vmovapd 0x100(%rsp),%xmm4
    1b32:	00 00 
    1b34:	c5 d9 c6 dc 01       	vshufpd $0x1,%xmm4,%xmm4,%xmm3
    1b39:	c5 f8 59 c3          	vmulps %xmm3,%xmm0,%xmm0
    1b3d:	c5 fc 10 ac 24 40 01 	vmovups 0x140(%rsp),%ymm5
    1b44:	00 00 
    1b46:	c4 e3 51 0c 9c 24 b0 	vblendps $0x7,0xb0(%rsp),%xmm5,%xmm3
    1b4d:	00 00 00 07 
    1b51:	c5 e0 c6 db 93       	vshufps $0x93,%xmm3,%xmm3,%xmm3
    1b56:	c4 e3 65 18 c9 01    	vinsertf128 $0x1,%xmm1,%ymm3,%ymm1
    1b5c:	c4 e3 65 18 d2 01    	vinsertf128 $0x1,%xmm2,%ymm3,%ymm2
    1b62:	c5 f5 c6 ca 02       	vshufpd $0x2,%ymm2,%ymm1,%ymm1
    1b67:	c4 e3 7d 19 ca 01    	vextractf128 $0x1,%ymm1,%xmm2
    1b6d:	c5 e8 c6 da a9       	vshufps $0xa9,%xmm2,%xmm2,%xmm3
    1b72:	c5 e8 c6 d2 3f       	vshufps $0x3f,%xmm2,%xmm2,%xmm2
    1b77:	c4 e3 65 18 d2 01    	vinsertf128 $0x1,%xmm2,%ymm3,%ymm2
    1b7d:	c4 e2 75 0c 0d 00 00 	vpermilps 0x0(%rip),%ymm1,%ymm1        # 1b86 <main+0x8f6>
    1b84:	00 00 
    1b86:	c4 e2 7d 18 35 00 00 	vbroadcastss 0x0(%rip),%ymm6        # 1b8f <main+0x8ff>
    1b8d:	00 00 
    1b8f:	c4 e3 4d 18 dc 01    	vinsertf128 $0x1,%xmm4,%ymm6,%ymm3
    1b95:	c5 f4 59 cb          	vmulps %ymm3,%ymm1,%ymm1
    1b99:	c5 fc 10 bc 24 20 01 	vmovups 0x120(%rsp),%ymm7
    1ba0:	00 00 
    1ba2:	c4 e3 45 18 9c 24 f0 	vinsertf128 $0x1,0xf0(%rsp),%ymm7,%ymm3
    1ba9:	00 00 00 01 
    1bad:	c5 ec 59 d3          	vmulps %ymm3,%ymm2,%ymm2
    1bb1:	c5 fc 10 a4 24 60 01 	vmovups 0x160(%rsp),%ymm4
    1bb8:	00 00 
    1bba:	c5 dc 58 d9          	vaddps %ymm1,%ymm4,%ymm3
    1bbe:	c5 dc 5c c9          	vsubps %ymm1,%ymm4,%ymm1
    1bc2:	c4 e3 65 0c c9 f0    	vblendps $0xf0,%ymm1,%ymm3,%ymm1
    1bc8:	c5 fc 11 8c 24 60 01 	vmovups %ymm1,0x160(%rsp)
    1bcf:	00 00 
    1bd1:	c5 f8 28 8c 24 c0 00 	vmovaps 0xc0(%rsp),%xmm1
    1bd8:	00 00 
    1bda:	c4 c3 71 21 c8 1c    	vinsertps $0x1c,%xmm8,%xmm1,%xmm1
    1be0:	c5 f0 c6 cd 24       	vshufps $0x24,%xmm5,%xmm1,%xmm1
    1be5:	c5 d4 5c d2          	vsubps %ymm2,%ymm5,%ymm2
    1be9:	c5 d0 c6 dd e9       	vshufps $0xe9,%xmm5,%xmm5,%xmm3
    1bee:	c5 e0 59 de          	vmulps %xmm6,%xmm3,%xmm3
    1bf2:	c5 78 28 4c 24 30    	vmovaps 0x30(%rsp),%xmm9
    1bf8:	c5 30 58 cb          	vaddps %xmm3,%xmm9,%xmm9
    1bfc:	c5 c8 59 9c 24 d0 00 	vmulps 0xd0(%rsp),%xmm6,%xmm3
    1c03:	00 00 
    1c05:	c5 78 28 ac 24 10 01 	vmovaps 0x110(%rsp),%xmm13
    1c0c:	00 00 
    1c0e:	c5 10 58 eb          	vaddps %xmm3,%xmm13,%xmm13
    1c12:	c5 f8 28 a4 24 90 00 	vmovaps 0x90(%rsp),%xmm4
    1c19:	00 00 
    1c1b:	c4 e3 71 21 cc 60    	vinsertps $0x60,%xmm4,%xmm1,%xmm1
    1c21:	c4 e3 49 21 df 10    	vinsertps $0x10,%xmm7,%xmm6,%xmm3
    1c27:	c5 f0 59 cb          	vmulps %xmm3,%xmm1,%xmm1
    1c2b:	c5 f0 58 e4          	vaddps %xmm4,%xmm1,%xmm4
    1c2f:	c5 f8 29 a4 24 90 00 	vmovaps %xmm4,0x90(%rsp)
    1c36:	00 00 
    1c38:	c5 80 58 c0          	vaddps %xmm0,%xmm15,%xmm0
    1c3c:	c5 80 59 ce          	vmulps %xmm6,%xmm15,%xmm1
    1c40:	c5 f8 28 64 24 20    	vmovaps 0x20(%rsp),%xmm4
    1c46:	c5 d8 58 e1          	vaddps %xmm1,%xmm4,%xmm4
    1c4a:	49 ff c6             	inc    %r14
    1c4d:	c5 fc 11 94 24 40 01 	vmovups %ymm2,0x140(%rsp)
    1c54:	00 00 
    1c56:	e9 d5 f6 ff ff       	jmp    1330 <main+0xa0>
    1c5b:	c5 f8 77             	vzeroupper
    1c5e:	e8 00 00 00 00       	call   1c63 <main+0x9d3>
    1c63:	31 c0                	xor    %eax,%eax
    1c65:	48 81 c4 80 01 00 00 	add    $0x180,%rsp
    1c6c:	5b                   	pop    %rbx
    1c6d:	41 5c                	pop    %r12
    1c6f:	41 5d                	pop    %r13
    1c71:	41 5e                	pop    %r14
    1c73:	41 5f                	pop    %r15
    1c75:	c5 f8 77             	vzeroupper
    1c78:	c3                   	ret
    1c79:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000001c80 <briev_rt_ctor>:
    1c80:	e9 00 00 00 00       	jmp    1c85 <briev_rt_ctor+0x5>
    1c85:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
    1c8c:	00 00 00 00 

0000000000001c90 <__rt_init>:
    1c90:	53                   	push   %rbx
    1c91:	48 81 ec 00 01 00 00 	sub    $0x100,%rsp
    1c98:	be 00 00 00 00       	mov    $0x0,%esi
    1c9d:	bf 02 00 00 00       	mov    $0x2,%edi
    1ca2:	e8 00 00 00 00       	call   1ca7 <__rt_init+0x17>
    1ca7:	be 00 00 00 00       	mov    $0x0,%esi
    1cac:	bf 0f 00 00 00       	mov    $0xf,%edi
    1cb1:	e8 00 00 00 00       	call   1cb6 <__rt_init+0x26>
    1cb6:	be 00 00 00 00       	mov    $0x0,%esi
    1cbb:	bf 01 00 00 00       	mov    $0x1,%edi
    1cc0:	e8 00 00 00 00       	call   1cc5 <__rt_init+0x35>
    1cc5:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    1cc9:	c5 fc 11 84 24 e0 00 	vmovups %ymm0,0xe0(%rsp)
    1cd0:	00 00 
    1cd2:	c5 fc 11 84 24 d0 00 	vmovups %ymm0,0xd0(%rsp)
    1cd9:	00 00 
    1cdb:	c5 fc 11 84 24 b0 00 	vmovups %ymm0,0xb0(%rsp)
    1ce2:	00 00 
    1ce4:	c5 fc 11 84 24 90 00 	vmovups %ymm0,0x90(%rsp)
    1ceb:	00 00 
    1ced:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
    1cf3:	48 c7 44 24 68 00 00 	movq   $0x0,0x68(%rsp)
    1cfa:	00 00 
    1cfc:	c7 84 24 f0 00 00 00 	movl   $0x4,0xf0(%rsp)
    1d03:	04 00 00 00 
    1d07:	c5 f8 77             	vzeroupper
    1d0a:	e8 00 00 00 00       	call   1d0f <__rt_init+0x7f>
    1d0f:	8d 78 01             	lea    0x1(%rax),%edi
    1d12:	48 8d 5c 24 68       	lea    0x68(%rsp),%rbx
    1d17:	48 89 de             	mov    %rbx,%rsi
    1d1a:	31 d2                	xor    %edx,%edx
    1d1c:	e8 00 00 00 00       	call   1d21 <__rt_init+0x91>
    1d21:	e8 00 00 00 00       	call   1d26 <__rt_init+0x96>
    1d26:	8d 78 02             	lea    0x2(%rax),%edi
    1d29:	48 89 de             	mov    %rbx,%rsi
    1d2c:	31 d2                	xor    %edx,%edx
    1d2e:	e8 00 00 00 00       	call   1d33 <__rt_init+0xa3>
    1d33:	e8 00 00 00 00       	call   1d38 <__rt_init+0xa8>
    1d38:	ff c0                	inc    %eax
    1d3a:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    1d3e:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
    1d43:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
    1d49:	89 44 24 08          	mov    %eax,0x8(%rsp)
    1d4d:	48 89 e6             	mov    %rsp,%rsi
    1d50:	ba 00 00 00 00       	mov    $0x0,%edx
    1d55:	31 ff                	xor    %edi,%edi
    1d57:	c5 f8 77             	vzeroupper
    1d5a:	e8 00 00 00 00       	call   1d5f <__rt_init+0xcf>
    1d5f:	85 c0                	test   %eax,%eax
    1d61:	75 27                	jne    1d8a <__rt_init+0xfa>
    1d63:	c4 e2 7d 1a 05 00 00 	vbroadcastf128 0x0(%rip),%ymm0        # 1d6c <__rt_init+0xdc>
    1d6a:	00 00 
    1d6c:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
    1d72:	48 8b 3d 00 00 00 00 	mov    0x0(%rip),%rdi        # 1d79 <__rt_init+0xe9>
    1d79:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
    1d7e:	31 f6                	xor    %esi,%esi
    1d80:	31 c9                	xor    %ecx,%ecx
    1d82:	c5 f8 77             	vzeroupper
    1d85:	e8 00 00 00 00       	call   1d8a <__rt_init+0xfa>
    1d8a:	e8 00 00 00 00       	call   1d8f <__rt_init+0xff>
    1d8f:	83 c0 02             	add    $0x2,%eax
    1d92:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    1d96:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
    1d9b:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
    1da1:	89 44 24 08          	mov    %eax,0x8(%rsp)
    1da5:	48 89 e6             	mov    %rsp,%rsi
    1da8:	ba 00 00 00 00       	mov    $0x0,%edx
    1dad:	31 ff                	xor    %edi,%edi
    1daf:	c5 f8 77             	vzeroupper
    1db2:	e8 00 00 00 00       	call   1db7 <__rt_init+0x127>
    1db7:	85 c0                	test   %eax,%eax
    1db9:	75 27                	jne    1de2 <__rt_init+0x152>
    1dbb:	c4 e2 7d 1a 05 00 00 	vbroadcastf128 0x0(%rip),%ymm0        # 1dc4 <__rt_init+0x134>
    1dc2:	00 00 
    1dc4:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
    1dca:	48 8b 3d 00 00 00 00 	mov    0x0(%rip),%rdi        # 1dd1 <__rt_init+0x141>
    1dd1:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
    1dd6:	31 f6                	xor    %esi,%esi
    1dd8:	31 c9                	xor    %ecx,%ecx
    1dda:	c5 f8 77             	vzeroupper
    1ddd:	e8 00 00 00 00       	call   1de2 <__rt_init+0x152>
    1de2:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 1de9 <__rt_init+0x159>
    1de9:	48 8b 38             	mov    (%rax),%rdi
    1dec:	31 f6                	xor    %esi,%esi
    1dee:	ba 01 00 00 00       	mov    $0x1,%edx
    1df3:	31 c9                	xor    %ecx,%ecx
    1df5:	e8 00 00 00 00       	call   1dfa <__rt_init+0x16a>
    1dfa:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1e01 <__rt_init+0x171>
    1e01:	48 81 c4 00 01 00 00 	add    $0x100,%rsp
    1e08:	5b                   	pop    %rbx
    1e09:	c3                   	ret
    1e0a:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

0000000000001e10 <handle_sigint>:
    1e10:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1e17 <handle_sigint+0x7>
    1e17:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1e1e <handle_sigint+0xe>
    1e1e:	c3                   	ret
    1e1f:	90                   	nop

0000000000001e20 <handle_sigterm>:
    1e20:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1e27 <handle_sigterm+0x7>
    1e27:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1e2e <handle_sigterm+0xe>
    1e2e:	c3                   	ret
    1e2f:	90                   	nop

0000000000001e30 <handle_sighup>:
    1e30:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1e37 <handle_sighup+0x7>
    1e37:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1e3e <handle_sighup+0xe>
    1e3e:	c3                   	ret
    1e3f:	90                   	nop

0000000000001e40 <handle_timer>:
    1e40:	48 ff 05 00 00 00 00 	incq   0x0(%rip)        # 1e47 <handle_timer+0x7>
    1e47:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1e4e <handle_timer+0xe>
    1e4e:	c3                   	ret
    1e4f:	90                   	nop

0000000000001e50 <__get_env_int>:
    1e50:	53                   	push   %rbx
    1e51:	48 83 ec 10          	sub    $0x10,%rsp
    1e55:	e8 00 00 00 00       	call   1e5a <__get_env_int+0xa>
    1e5a:	48 85 c0             	test   %rax,%rax
    1e5d:	74 32                	je     1e91 <__get_env_int+0x41>
    1e5f:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
    1e66:	00 00 
    1e68:	48 8d 74 24 08       	lea    0x8(%rsp),%rsi
    1e6d:	48 89 c7             	mov    %rax,%rdi
    1e70:	ba 0a 00 00 00       	mov    $0xa,%edx
    1e75:	48 89 c3             	mov    %rax,%rbx
    1e78:	e8 00 00 00 00       	call   1e7d <__get_env_int+0x2d>
    1e7d:	48 89 c1             	mov    %rax,%rcx
    1e80:	31 c0                	xor    %eax,%eax
    1e82:	48 39 5c 24 08       	cmp    %rbx,0x8(%rsp)
    1e87:	48 0f 45 c1          	cmovne %rcx,%rax
    1e8b:	48 83 c4 10          	add    $0x10,%rsp
    1e8f:	5b                   	pop    %rbx
    1e90:	c3                   	ret
    1e91:	31 c0                	xor    %eax,%eax
    1e93:	48 83 c4 10          	add    $0x10,%rsp
    1e97:	5b                   	pop    %rbx
    1e98:	c3                   	ret
    1e99:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000001ea0 <__rt_wait>:
    1ea0:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
    1ea7:	8b 3d 00 00 00 00    	mov    0x0(%rip),%edi        # 1ead <__rt_wait+0xd>
    1ead:	85 ff                	test   %edi,%edi
    1eaf:	79 3f                	jns    1ef0 <__rt_wait+0x50>
    1eb1:	31 ff                	xor    %edi,%edi
    1eb3:	e8 00 00 00 00       	call   1eb8 <__rt_wait+0x18>
    1eb8:	89 05 00 00 00 00    	mov    %eax,0x0(%rip)        # 1ebe <__rt_wait+0x1e>
    1ebe:	85 c0                	test   %eax,%eax
    1ec0:	0f 88 d5 00 00 00    	js     1f9b <__rt_wait+0xfb>
    1ec6:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
    1ecd:	00 
    1ece:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
    1ed5:	00 00 
    1ed7:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
    1edc:	89 c7                	mov    %eax,%edi
    1ede:	be 01 00 00 00       	mov    $0x1,%esi
    1ee3:	31 d2                	xor    %edx,%edx
    1ee5:	e8 00 00 00 00       	call   1eea <__rt_wait+0x4a>
    1eea:	8b 3d 00 00 00 00    	mov    0x0(%rip),%edi        # 1ef0 <__rt_wait+0x50>
    1ef0:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
    1ef5:	ba 40 00 00 00       	mov    $0x40,%edx
    1efa:	b9 64 00 00 00       	mov    $0x64,%ecx
    1eff:	e8 00 00 00 00       	call   1f04 <__rt_wait+0x64>
    1f04:	85 c0                	test   %eax,%eax
    1f06:	0f 8e ef 00 00 00    	jle    1ffb <__rt_wait+0x15b>
    1f0c:	89 c1                	mov    %eax,%ecx
    1f0e:	83 f8 01             	cmp    $0x1,%eax
    1f11:	75 1e                	jne    1f31 <__rt_wait+0x91>
    1f13:	31 c0                	xor    %eax,%eax
    1f15:	f6 c1 01             	test   $0x1,%cl
    1f18:	74 0f                	je     1f29 <__rt_wait+0x89>
    1f1a:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
    1f1e:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
    1f23:	0f 84 e1 00 00 00    	je     200a <__rt_wait+0x16a>
    1f29:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
    1f30:	c3                   	ret
    1f31:	89 c8                	mov    %ecx,%eax
    1f33:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
    1f38:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
    1f3d:	48 89 c6             	mov    %rax,%rsi
    1f40:	eb 18                	jmp    1f5a <__rt_wait+0xba>
    1f42:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1f49:	1f 84 00 00 00 00 00 
    1f50:	48 83 c2 18          	add    $0x18,%rdx
    1f54:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
    1f58:	74 bb                	je     1f15 <__rt_wait+0x75>
    1f5a:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
    1f5e:	75 20                	jne    1f80 <__rt_wait+0xe0>
    1f60:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
    1f64:	74 1a                	je     1f80 <__rt_wait+0xe0>
    1f66:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1f6d <__rt_wait+0xcd>
    1f6d:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1f74 <__rt_wait+0xd4>
    1f74:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
    1f7b:	00 00 00 00 00 
    1f80:	83 3a 00             	cmpl   $0x0,(%rdx)
    1f83:	75 cb                	jne    1f50 <__rt_wait+0xb0>
    1f85:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
    1f89:	74 c5                	je     1f50 <__rt_wait+0xb0>
    1f8b:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1f92 <__rt_wait+0xf2>
    1f92:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1f99 <__rt_wait+0xf9>
    1f99:	eb b5                	jmp    1f50 <__rt_wait+0xb0>
    1f9b:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    1f9f:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
    1fa5:	c5 fc 11 44 24 58    	vmovups %ymm0,0x58(%rsp)
    1fab:	c5 fc 11 44 24 38    	vmovups %ymm0,0x38(%rsp)
    1fb1:	c5 fc 11 44 24 18    	vmovups %ymm0,0x18(%rsp)
    1fb7:	48 c7 44 24 10 01 00 	movq   $0x1,0x10(%rsp)
    1fbe:	00 00 
    1fc0:	c5 f8 10 05 00 00 00 	vmovups 0x0(%rip),%xmm0        # 1fc8 <__rt_wait+0x128>
    1fc7:	00 
    1fc8:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
    1fcd:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
    1fd2:	49 89 e0             	mov    %rsp,%r8
    1fd5:	bf 01 00 00 00       	mov    $0x1,%edi
    1fda:	31 d2                	xor    %edx,%edx
    1fdc:	31 c9                	xor    %ecx,%ecx
    1fde:	c5 f8 77             	vzeroupper
    1fe1:	e8 00 00 00 00       	call   1fe6 <__rt_wait+0x146>
    1fe6:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
    1feb:	74 0e                	je     1ffb <__rt_wait+0x15b>
    1fed:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1ff4 <__rt_wait+0x154>
    1ff4:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1ffb <__rt_wait+0x15b>
    1ffb:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 2002 <__rt_wait+0x162>
    2002:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
    2009:	c3                   	ret
    200a:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
    200f:	0f 84 14 ff ff ff    	je     1f29 <__rt_wait+0x89>
    2015:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 201c <__rt_wait+0x17c>
    201c:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 2023 <__rt_wait+0x183>
    2023:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
    202a:	c3                   	ret
    202b:	0f 1f 44 00 00       	nopl   0x0(%rax,%rax,1)

0000000000002030 <__rt_poll>:
    2030:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
    2037:	8b 3d 00 00 00 00    	mov    0x0(%rip),%edi        # 203d <__rt_poll+0xd>
    203d:	85 ff                	test   %edi,%edi
    203f:	79 3f                	jns    2080 <__rt_poll+0x50>
    2041:	31 ff                	xor    %edi,%edi
    2043:	e8 00 00 00 00       	call   2048 <__rt_poll+0x18>
    2048:	89 05 00 00 00 00    	mov    %eax,0x0(%rip)        # 204e <__rt_poll+0x1e>
    204e:	85 c0                	test   %eax,%eax
    2050:	0f 88 d5 00 00 00    	js     212b <__rt_poll+0xfb>
    2056:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
    205d:	00 
    205e:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
    2065:	00 00 
    2067:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
    206c:	89 c7                	mov    %eax,%edi
    206e:	be 01 00 00 00       	mov    $0x1,%esi
    2073:	31 d2                	xor    %edx,%edx
    2075:	e8 00 00 00 00       	call   207a <__rt_poll+0x4a>
    207a:	8b 3d 00 00 00 00    	mov    0x0(%rip),%edi        # 2080 <__rt_poll+0x50>
    2080:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
    2085:	ba 40 00 00 00       	mov    $0x40,%edx
    208a:	31 c9                	xor    %ecx,%ecx
    208c:	e8 00 00 00 00       	call   2091 <__rt_poll+0x61>
    2091:	85 c0                	test   %eax,%eax
    2093:	7e 1d                	jle    20b2 <__rt_poll+0x82>
    2095:	89 c1                	mov    %eax,%ecx
    2097:	83 f8 01             	cmp    $0x1,%eax
    209a:	75 25                	jne    20c1 <__rt_poll+0x91>
    209c:	31 c0                	xor    %eax,%eax
    209e:	f6 c1 01             	test   $0x1,%cl
    20a1:	74 0f                	je     20b2 <__rt_poll+0x82>
    20a3:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
    20a7:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
    20ac:	0f 84 cd 00 00 00    	je     217f <__rt_poll+0x14f>
    20b2:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 20b9 <__rt_poll+0x89>
    20b9:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
    20c0:	c3                   	ret
    20c1:	89 c8                	mov    %ecx,%eax
    20c3:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
    20c8:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
    20cd:	48 89 c6             	mov    %rax,%rsi
    20d0:	eb 18                	jmp    20ea <__rt_poll+0xba>
    20d2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    20d9:	1f 84 00 00 00 00 00 
    20e0:	48 83 c2 18          	add    $0x18,%rdx
    20e4:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
    20e8:	74 b4                	je     209e <__rt_poll+0x6e>
    20ea:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
    20ee:	75 20                	jne    2110 <__rt_poll+0xe0>
    20f0:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
    20f4:	74 1a                	je     2110 <__rt_poll+0xe0>
    20f6:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 20fd <__rt_poll+0xcd>
    20fd:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 2104 <__rt_poll+0xd4>
    2104:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
    210b:	00 00 00 00 00 
    2110:	83 3a 00             	cmpl   $0x0,(%rdx)
    2113:	75 cb                	jne    20e0 <__rt_poll+0xb0>
    2115:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
    2119:	74 c5                	je     20e0 <__rt_poll+0xb0>
    211b:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 2122 <__rt_poll+0xf2>
    2122:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 2129 <__rt_poll+0xf9>
    2129:	eb b5                	jmp    20e0 <__rt_poll+0xb0>
    212b:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    212f:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
    2135:	c5 fc 11 44 24 58    	vmovups %ymm0,0x58(%rsp)
    213b:	c5 fc 11 44 24 38    	vmovups %ymm0,0x38(%rsp)
    2141:	c5 fc 11 44 24 18    	vmovups %ymm0,0x18(%rsp)
    2147:	48 c7 44 24 10 01 00 	movq   $0x1,0x10(%rsp)
    214e:	00 00 
    2150:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    2154:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
    2159:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
    215e:	49 89 e0             	mov    %rsp,%r8
    2161:	bf 01 00 00 00       	mov    $0x1,%edi
    2166:	31 d2                	xor    %edx,%edx
    2168:	31 c9                	xor    %ecx,%ecx
    216a:	c5 f8 77             	vzeroupper
    216d:	e8 00 00 00 00       	call   2172 <__rt_poll+0x142>
    2172:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
    2177:	0f 84 35 ff ff ff    	je     20b2 <__rt_poll+0x82>
    217d:	eb 0b                	jmp    218a <__rt_poll+0x15a>
    217f:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
    2184:	0f 84 28 ff ff ff    	je     20b2 <__rt_poll+0x82>
    218a:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 2191 <__rt_poll+0x161>
    2191:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 2198 <__rt_poll+0x168>
    2198:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 219f <__rt_poll+0x16f>
    219f:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
    21a6:	c3                   	ret
    21a7:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
    21ae:	00 00 

00000000000021b0 <__wait_for_event>:
    21b0:	e9 00 00 00 00       	jmp    21b5 <__wait_for_event+0x5>
    21b5:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
    21bc:	00 00 00 00 

00000000000021c0 <__print>:
    21c0:	50                   	push   %rax
    21c1:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 21c8 <__print+0x8>
    21c8:	48 8b 30             	mov    (%rax),%rsi
    21cb:	e8 00 00 00 00       	call   21d0 <__print+0x10>
    21d0:	b8 01 00 00 00       	mov    $0x1,%eax
    21d5:	59                   	pop    %rcx
    21d6:	c3                   	ret
    21d7:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
    21de:	00 00 

00000000000021e0 <__print_int>:
    21e0:	50                   	push   %rax
    21e1:	48 89 fa             	mov    %rdi,%rdx
    21e4:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 21eb <__print_int+0xb>
    21eb:	48 8b 38             	mov    (%rax),%rdi
    21ee:	be 00 00 00 00       	mov    $0x0,%esi
    21f3:	31 c0                	xor    %eax,%eax
    21f5:	e8 00 00 00 00       	call   21fa <__print_int+0x1a>
    21fa:	b8 01 00 00 00       	mov    $0x1,%eax
    21ff:	59                   	pop    %rcx
    2200:	c3                   	ret
    2201:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    2208:	0f 1f 84 00 00 00 00 
    220f:	00 

0000000000002210 <__print_float>:
    2210:	50                   	push   %rax
    2211:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 2218 <__print_float+0x8>
    2218:	48 8b 38             	mov    (%rax),%rdi
    221b:	c5 fa 5a c0          	vcvtss2sd %xmm0,%xmm0,%xmm0
    221f:	be 00 00 00 00       	mov    $0x0,%esi
    2224:	b0 01                	mov    $0x1,%al
    2226:	e8 00 00 00 00       	call   222b <__print_float+0x1b>
    222b:	b8 01 00 00 00       	mov    $0x1,%eax
    2230:	59                   	pop    %rcx
    2231:	c3                   	ret
    2232:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    2239:	1f 84 00 00 00 00 00 

0000000000002240 <__sqrtf>:
    2240:	c5 f0 57 c9          	vxorps %xmm1,%xmm1,%xmm1
    2244:	c5 f8 2e c1          	vucomiss %xmm1,%xmm0
    2248:	0f 82 00 00 00 00    	jb     224e <__sqrtf+0xe>
    224e:	c5 fa 51 c0          	vsqrtss %xmm0,%xmm0,%xmm0
    2252:	c3                   	ret
    2253:	66 66 66 66 2e 0f 1f 	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    225a:	84 00 00 00 00 00 

0000000000002260 <__exit>:
    2260:	50                   	push   %rax
    2261:	31 ff                	xor    %edi,%edi
    2263:	e8 00 00 00 00       	call   2268 <__exit+0x8>
    2268:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
    226f:	00 

0000000000002270 <__read_stdin>:
    2270:	48 89 f2             	mov    %rsi,%rdx
    2273:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 227a <__read_stdin+0xa>
    227a:	48 8b 08             	mov    (%rax),%rcx
    227d:	be 01 00 00 00       	mov    $0x1,%esi
    2282:	e9 00 00 00 00       	jmp    2287 <__read_stdin+0x17>
    2287:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
    228e:	00 00 

0000000000002290 <__putchar>:
    2290:	53                   	push   %rbx
    2291:	48 89 fb             	mov    %rdi,%rbx
    2294:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 229b <__putchar+0xb>
    229b:	48 8b 30             	mov    (%rax),%rsi
    229e:	e8 00 00 00 00       	call   22a3 <__putchar+0x13>
    22a3:	48 89 d8             	mov    %rbx,%rax
    22a6:	5b                   	pop    %rbx
    22a7:	c3                   	ret
    22a8:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
    22af:	00 

00000000000022b0 <briev_thread_pool_init>:
    22b0:	c3                   	ret
    22b1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    22b8:	0f 1f 84 00 00 00 00 
    22bf:	00 

00000000000022c0 <briev_barrier_release>:
    22c0:	c3                   	ret
    22c1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    22c8:	0f 1f 84 00 00 00 00 
    22cf:	00 

00000000000022d0 <briev_barrier_wait>:
    22d0:	c3                   	ret
    22d1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    22d8:	0f 1f 84 00 00 00 00 
    22df:	00 

00000000000022e0 <briev_thread_pool_shutdown>:
    22e0:	c3                   	ret
