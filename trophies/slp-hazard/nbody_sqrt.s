
trophies/slp-hazard/nbody_sqrt.o:     file format elf64-x86-64


Disassembly of section .text:

0000000000000000 <simulate>:
       0:	53                   	push   %rbx
       1:	48 81 ec 70 01 00 00 	sub    $0x170,%rsp
       8:	48 89 fb             	mov    %rdi,%rbx
       b:	c5 7a 10 7f 30       	vmovss 0x30(%rdi),%xmm15
      10:	c5 fa 10 47 48       	vmovss 0x48(%rdi),%xmm0
      15:	c5 f8 29 44 24 30    	vmovaps %xmm0,0x30(%rsp)
      1b:	c5 fa 10 67 60       	vmovss 0x60(%rdi),%xmm4
      20:	c5 f8 29 64 24 70    	vmovaps %xmm4,0x70(%rsp)
      26:	c5 7a 10 4f 78       	vmovss 0x78(%rdi),%xmm9
      2b:	c5 fa 5c d4          	vsubss %xmm4,%xmm0,%xmm2
      2f:	c5 ea 59 c2          	vmulss %xmm2,%xmm2,%xmm0
      33:	c4 e3 01 21 cc 10    	vinsertps $0x10,%xmm4,%xmm15,%xmm1
      39:	c4 c3 59 21 d9 10    	vinsertps $0x10,%xmm9,%xmm4,%xmm3
      3f:	c5 7b 10 6f 40       	vmovsd 0x40(%rdi),%xmm13
      44:	c5 fb 10 67 58       	vmovsd 0x58(%rdi),%xmm4
      49:	c5 f8 29 64 24 10    	vmovaps %xmm4,0x10(%rsp)
      4f:	c5 90 5c e4          	vsubps %xmm4,%xmm13,%xmm4
      53:	c5 78 29 6c 24 50    	vmovaps %xmm13,0x50(%rsp)
      59:	c5 da 59 ec          	vmulss %xmm4,%xmm4,%xmm5
      5d:	c5 d2 58 c0          	vaddss %xmm0,%xmm5,%xmm0
      61:	c5 fa 16 ec          	vmovshdup %xmm4,%xmm5
      65:	c5 d2 59 f5          	vmulss %xmm5,%xmm5,%xmm6
      69:	c5 ca 58 c0          	vaddss %xmm0,%xmm6,%xmm0
      6d:	c5 fa 59 f0          	vmulss %xmm0,%xmm0,%xmm6
      71:	c5 ca 59 c0          	vmulss %xmm0,%xmm6,%xmm0
      75:	c5 fa 52 f0          	vrsqrtss %xmm0,%xmm0,%xmm6
      79:	c5 fa 59 c6          	vmulss %xmm6,%xmm0,%xmm0
      7d:	c5 fa 59 c6          	vmulss %xmm6,%xmm0,%xmm0
      81:	c5 7a 10 15 00 00 00 	vmovss 0x0(%rip),%xmm10        # 89 <simulate+0x89>
      88:	00 
      89:	c5 aa 58 c0          	vaddss %xmm0,%xmm10,%xmm0
      8d:	c5 7a 10 35 00 00 00 	vmovss 0x0(%rip),%xmm14        # 95 <simulate+0x95>
      94:	00 
      95:	c5 8a 59 f6          	vmulss %xmm6,%xmm14,%xmm6
      99:	c5 ca 59 f0          	vmulss %xmm0,%xmm6,%xmm6
      9d:	c5 ca 59 3d 00 00 00 	vmulss 0x0(%rip),%xmm6,%xmm7        # a5 <simulate+0xa5>
      a4:	00 
      a5:	c5 70 5c c3          	vsubps %xmm3,%xmm1,%xmm8
      a9:	c5 fa 12 cf          	vmovsldup %xmm7,%xmm1
      ad:	c5 f0 59 c4          	vmulps %xmm4,%xmm1,%xmm0
      b1:	c5 f8 29 84 24 20 01 	vmovaps %xmm0,0x120(%rsp)
      b8:	00 00 
      ba:	c5 c2 59 c2          	vmulss %xmm2,%xmm7,%xmm0
      be:	c5 fa 11 84 24 30 01 	vmovss %xmm0,0x130(%rsp)
      c5:	00 00 
      c7:	c5 ca 59 1d 00 00 00 	vmulss 0x0(%rip),%xmm6,%xmm3        # cf <simulate+0xcf>
      ce:	00 
      cf:	c5 e2 59 c4          	vmulss %xmm4,%xmm3,%xmm0
      d3:	c5 fa 11 04 24       	vmovss %xmm0,(%rsp)
      d8:	c5 e2 59 e5          	vmulss %xmm5,%xmm3,%xmm4
      dc:	c5 e2 59 d2          	vmulss %xmm2,%xmm3,%xmm2
      e0:	c4 e3 59 21 c2 10    	vinsertps $0x10,%xmm2,%xmm4,%xmm0
      e6:	c5 f8 29 84 24 40 01 	vmovaps %xmm0,0x140(%rsp)
      ed:	00 00 
      ef:	c5 fb 10 47 70       	vmovsd 0x70(%rdi),%xmm0
      f4:	c5 fb 12 67 28       	vmovddup 0x28(%rdi),%xmm4
      f9:	c5 d8 5c d8          	vsubps %xmm0,%xmm4,%xmm3
      fd:	c5 f8 28 c8          	vmovaps %xmm0,%xmm1
     101:	c5 f8 29 44 24 20    	vmovaps %xmm0,0x20(%rsp)
     107:	c5 e0 59 d3          	vmulps %xmm3,%xmm3,%xmm2
     10b:	c5 fa 16 ea          	vmovshdup %xmm2,%xmm5
     10f:	c5 d2 58 d2          	vaddss %xmm2,%xmm5,%xmm2
     113:	c5 78 29 c8          	vmovaps %xmm9,%xmm0
     117:	c5 78 29 4c 24 60    	vmovaps %xmm9,0x60(%rsp)
     11d:	c4 c1 02 5c e9       	vsubss %xmm9,%xmm15,%xmm5
     122:	c5 d2 59 f5          	vmulss %xmm5,%xmm5,%xmm6
     126:	c5 ea 58 d6          	vaddss %xmm6,%xmm2,%xmm2
     12a:	c5 ea 59 f2          	vmulss %xmm2,%xmm2,%xmm6
     12e:	c5 ca 59 d2          	vmulss %xmm2,%xmm6,%xmm2
     132:	c5 ea 52 f2          	vrsqrtss %xmm2,%xmm2,%xmm6
     136:	c5 ea 59 d6          	vmulss %xmm6,%xmm2,%xmm2
     13a:	c5 ea 59 d6          	vmulss %xmm6,%xmm2,%xmm2
     13e:	c5 aa 58 d2          	vaddss %xmm2,%xmm10,%xmm2
     142:	c5 8a 59 f6          	vmulss %xmm6,%xmm14,%xmm6
     146:	c5 ca 59 f2          	vmulss %xmm2,%xmm6,%xmm6
     14a:	c5 fa 10 3d 00 00 00 	vmovss 0x0(%rip),%xmm7        # 152 <simulate+0x152>
     151:	00 
     152:	c5 4a 59 cf          	vmulss %xmm7,%xmm6,%xmm9
     156:	c4 c1 7a 12 d1       	vmovsldup %xmm9,%xmm2
     15b:	c5 e8 59 d3          	vmulps %xmm3,%xmm2,%xmm2
     15f:	c5 f8 29 94 24 10 01 	vmovaps %xmm2,0x110(%rsp)
     166:	00 00 
     168:	c5 fa 10 15 00 00 00 	vmovss 0x0(%rip),%xmm2        # 170 <simulate+0x170>
     16f:	00 
     170:	c5 4a 59 da          	vmulss %xmm2,%xmm6,%xmm11
     174:	c4 c1 7a 12 f3       	vmovsldup %xmm11,%xmm6
     179:	c5 c8 59 f3          	vmulps %xmm3,%xmm6,%xmm6
     17d:	c5 32 59 cd          	vmulss %xmm5,%xmm9,%xmm9
     181:	c5 a2 59 dd          	vmulss %xmm5,%xmm11,%xmm3
     185:	c4 e2 79 18 6f 18    	vbroadcastss 0x18(%rdi),%xmm5
     18b:	c5 f8 29 6c 24 40    	vmovaps %xmm5,0x40(%rsp)
     191:	c5 d2 5c e8          	vsubss %xmm0,%xmm5,%xmm5
     195:	c5 52 59 dd          	vmulss %xmm5,%xmm5,%xmm11
     199:	c5 fb 10 47 10       	vmovsd 0x10(%rdi),%xmm0
     19e:	c5 f8 29 84 24 60 01 	vmovaps %xmm0,0x160(%rsp)
     1a5:	00 00 
     1a7:	c5 f8 5c c1          	vsubps %xmm1,%xmm0,%xmm0
     1ab:	c5 fa 59 c8          	vmulss %xmm0,%xmm0,%xmm1
     1af:	c5 a2 58 c9          	vaddss %xmm1,%xmm11,%xmm1
     1b3:	c5 7a 16 d8          	vmovshdup %xmm0,%xmm11
     1b7:	c4 41 22 59 e3       	vmulss %xmm11,%xmm11,%xmm12
     1bc:	c5 9a 58 c9          	vaddss %xmm1,%xmm12,%xmm1
     1c0:	c5 72 59 e1          	vmulss %xmm1,%xmm1,%xmm12
     1c4:	c5 9a 59 c9          	vmulss %xmm1,%xmm12,%xmm1
     1c8:	c5 72 52 e1          	vrsqrtss %xmm1,%xmm1,%xmm12
     1cc:	c5 9a 59 c9          	vmulss %xmm1,%xmm12,%xmm1
     1d0:	c5 9a 59 c9          	vmulss %xmm1,%xmm12,%xmm1
     1d4:	c5 aa 58 c9          	vaddss %xmm1,%xmm10,%xmm1
     1d8:	c4 41 1a 59 e6       	vmulss %xmm14,%xmm12,%xmm12
     1dd:	c5 9a 59 c9          	vmulss %xmm1,%xmm12,%xmm1
     1e1:	c5 f2 59 ff          	vmulss %xmm7,%xmm1,%xmm7
     1e5:	c5 22 59 df          	vmulss %xmm7,%xmm11,%xmm11
     1e9:	c5 42 59 e5          	vmulss %xmm5,%xmm7,%xmm12
     1ed:	c4 43 21 21 dc 10    	vinsertps $0x10,%xmm12,%xmm11,%xmm11
     1f3:	c5 78 29 9c 24 00 01 	vmovaps %xmm11,0x100(%rsp)
     1fa:	00 00 
     1fc:	c5 c2 59 f8          	vmulss %xmm0,%xmm7,%xmm7
     200:	c5 fa 11 bc 24 f0 00 	vmovss %xmm7,0xf0(%rsp)
     207:	00 00 
     209:	c5 f2 59 0d 00 00 00 	vmulss 0x0(%rip),%xmm1,%xmm1        # 211 <simulate+0x211>
     210:	00 
     211:	c5 fa 12 f9          	vmovsldup %xmm1,%xmm7
     215:	c5 c0 59 c0          	vmulps %xmm0,%xmm7,%xmm0
     219:	c5 fb 10 7f 7c       	vmovsd 0x7c(%rdi),%xmm7
     21e:	c5 c0 58 c0          	vaddps %xmm0,%xmm7,%xmm0
     222:	c5 f8 58 c6          	vaddps %xmm6,%xmm0,%xmm0
     226:	c5 fb 10 77 68       	vmovsd 0x68(%rdi),%xmm6
     22b:	c5 c8 16 c0          	vmovlhps %xmm0,%xmm6,%xmm0
     22f:	c5 f8 29 84 24 b0 00 	vmovaps %xmm0,0xb0(%rsp)
     236:	00 00 
     238:	c5 f2 59 c5          	vmulss %xmm5,%xmm1,%xmm0
     23c:	c5 fa 58 87 84 00 00 	vaddss 0x84(%rdi),%xmm0,%xmm0
     243:	00 
     244:	c5 fa 58 c3          	vaddss %xmm3,%xmm0,%xmm0
     248:	c5 fa 11 84 24 a0 00 	vmovss %xmm0,0xa0(%rsp)
     24f:	00 00 
     251:	c4 c1 58 5c c5       	vsubps %xmm13,%xmm4,%xmm0
     256:	c5 f8 59 c8          	vmulps %xmm0,%xmm0,%xmm1
     25a:	c5 fa 16 d9          	vmovshdup %xmm1,%xmm3
     25e:	c5 e2 58 c9          	vaddss %xmm1,%xmm3,%xmm1
     262:	c5 78 28 5c 24 30    	vmovaps 0x30(%rsp),%xmm11
     268:	c4 c1 02 5c db       	vsubss %xmm11,%xmm15,%xmm3
     26d:	c5 78 29 bc 24 90 00 	vmovaps %xmm15,0x90(%rsp)
     274:	00 00 
     276:	c5 e2 59 eb          	vmulss %xmm3,%xmm3,%xmm5
     27a:	c5 f2 58 cd          	vaddss %xmm5,%xmm1,%xmm1
     27e:	c5 f2 59 e9          	vmulss %xmm1,%xmm1,%xmm5
     282:	c5 d2 59 c9          	vmulss %xmm1,%xmm5,%xmm1
     286:	c5 f2 52 e9          	vrsqrtss %xmm1,%xmm1,%xmm5
     28a:	c5 f2 59 cd          	vmulss %xmm5,%xmm1,%xmm1
     28e:	c5 f2 59 cd          	vmulss %xmm5,%xmm1,%xmm1
     292:	c5 aa 58 c9          	vaddss %xmm1,%xmm10,%xmm1
     296:	c5 8a 59 ed          	vmulss %xmm5,%xmm14,%xmm5
     29a:	c5 d2 59 c9          	vmulss %xmm1,%xmm5,%xmm1
     29e:	c5 f2 59 2d 00 00 00 	vmulss 0x0(%rip),%xmm1,%xmm5        # 2a6 <simulate+0x2a6>
     2a5:	00 
     2a6:	c5 d2 59 f3          	vmulss %xmm3,%xmm5,%xmm6
     2aa:	c5 32 58 d6          	vaddss %xmm6,%xmm9,%xmm10
     2ae:	c5 f2 59 ca          	vmulss %xmm2,%xmm1,%xmm1
     2b2:	c5 fa 12 d1          	vmovsldup %xmm1,%xmm2
     2b6:	c5 e8 59 d0          	vmulps %xmm0,%xmm2,%xmm2
     2ba:	c5 fb 10 77 4c       	vmovsd 0x4c(%rdi),%xmm6
     2bf:	c5 c8 58 d2          	vaddps %xmm2,%xmm6,%xmm2
     2c3:	c5 f8 29 94 24 c0 00 	vmovaps %xmm2,0xc0(%rsp)
     2ca:	00 00 
     2cc:	c5 f2 59 cb          	vmulss %xmm3,%xmm1,%xmm1
     2d0:	c5 fa 11 8c 24 d0 00 	vmovss %xmm1,0xd0(%rsp)
     2d7:	00 00 
     2d9:	c5 fa 12 cd          	vmovsldup %xmm5,%xmm1
     2dd:	c5 f0 59 f8          	vmulps %xmm0,%xmm1,%xmm7
     2e1:	c5 f8 28 4c 24 10    	vmovaps 0x10(%rsp),%xmm1
     2e7:	c5 d8 5c e1          	vsubps %xmm1,%xmm4,%xmm4
     2eb:	c5 f0 5c 5c 24 20    	vsubps 0x20(%rsp),%xmm1,%xmm3
     2f1:	c4 e3 59 21 c3 1c    	vinsertps $0x1c,%xmm3,%xmm4,%xmm0
     2f7:	c5 f8 59 c0          	vmulps %xmm0,%xmm0,%xmm0
     2fb:	c4 c1 38 59 c8       	vmulps %xmm8,%xmm8,%xmm1
     300:	c5 f8 58 c1          	vaddps %xmm1,%xmm0,%xmm0
     304:	c5 fa 16 cc          	vmovshdup %xmm4,%xmm1
     308:	c4 e3 71 0c d3 02    	vblendps $0x2,%xmm3,%xmm1,%xmm2
     30e:	c5 e8 59 d2          	vmulps %xmm2,%xmm2,%xmm2
     312:	c5 e8 58 c0          	vaddps %xmm0,%xmm2,%xmm0
     316:	c5 f8 59 d0          	vmulps %xmm0,%xmm0,%xmm2
     31a:	c5 e8 59 c0          	vmulps %xmm0,%xmm2,%xmm0
     31e:	c5 f8 52 d0          	vrsqrtps %xmm0,%xmm2
     322:	c5 f8 59 c2          	vmulps %xmm2,%xmm0,%xmm0
     326:	c5 f8 59 c2          	vmulps %xmm2,%xmm0,%xmm0
     32a:	c5 f8 58 05 00 00 00 	vaddps 0x0(%rip),%xmm0,%xmm0        # 332 <simulate+0x332>
     331:	00 
     332:	c5 e8 59 15 00 00 00 	vmulps 0x0(%rip),%xmm2,%xmm2        # 33a <simulate+0x33a>
     339:	00 
     33a:	c5 e8 59 c0          	vmulps %xmm0,%xmm2,%xmm0
     33e:	c4 62 79 18 35 00 00 	vbroadcastss 0x0(%rip),%xmm14        # 347 <simulate+0x347>
     345:	00 00 
     347:	c5 88 59 d0          	vmulps %xmm0,%xmm14,%xmm2
     34b:	c5 fa 16 ea          	vmovshdup %xmm2,%xmm5
     34f:	c5 52 59 25 00 00 00 	vmulss 0x0(%rip),%xmm5,%xmm12        # 357 <simulate+0x357>
     356:	00 
     357:	c5 f8 59 05 00 00 00 	vmulps 0x0(%rip),%xmm0,%xmm0        # 35f <simulate+0x35f>
     35e:	00 
     35f:	c4 c1 7a 12 ec       	vmovsldup %xmm12,%xmm5
     364:	c4 63 39 21 cb 4c    	vinsertps $0x4c,%xmm3,%xmm8,%xmm9
     36a:	c5 b0 59 ed          	vmulps %xmm5,%xmm9,%xmm5
     36e:	c5 f8 29 ac 24 e0 00 	vmovaps %xmm5,0xe0(%rsp)
     375:	00 00 
     377:	c4 c1 7a 16 e8       	vmovshdup %xmm8,%xmm5
     37c:	c5 7a 16 c8          	vmovshdup %xmm0,%xmm9
     380:	c5 b2 59 ed          	vmulss %xmm5,%xmm9,%xmm5
     384:	c5 fa 11 6c 24 0c    	vmovss %xmm5,0xc(%rsp)
     38a:	c5 ea 59 15 00 00 00 	vmulss 0x0(%rip),%xmm2,%xmm2        # 392 <simulate+0x392>
     391:	00 
     392:	c5 fa 12 ea          	vmovsldup %xmm2,%xmm5
     396:	c5 d0 59 ec          	vmulps %xmm4,%xmm5,%xmm5
     39a:	c5 c0 58 ed          	vaddps %xmm5,%xmm7,%xmm5
     39e:	c5 f8 29 ac 24 80 00 	vmovaps %xmm5,0x80(%rsp)
     3a5:	00 00 
     3a7:	c5 ba 59 d2          	vmulss %xmm2,%xmm8,%xmm2
     3ab:	c5 aa 58 ea          	vaddss %xmm2,%xmm10,%xmm5
     3af:	c5 fa 59 d4          	vmulss %xmm4,%xmm0,%xmm2
     3b3:	c5 ea 58 14 24       	vaddss (%rsp),%xmm2,%xmm2
     3b8:	c5 fa 11 54 24 04    	vmovss %xmm2,0x4(%rsp)
     3be:	c4 c1 70 14 c8       	vunpcklps %xmm8,%xmm1,%xmm1
     3c3:	c5 9a 59 d3          	vmulss %xmm3,%xmm12,%xmm2
     3c7:	c5 fa 11 54 24 08    	vmovss %xmm2,0x8(%rsp)
     3cd:	c5 f0 16 cb          	vmovlhps %xmm3,%xmm1,%xmm1
     3d1:	c5 f8 c6 c0 50       	vshufps $0x50,%xmm0,%xmm0,%xmm0
     3d6:	c5 f8 59 c1          	vmulps %xmm1,%xmm0,%xmm0
     3da:	c5 f8 29 84 24 50 01 	vmovaps %xmm0,0x150(%rsp)
     3e1:	00 00 
     3e3:	c5 78 29 df          	vmovaps %xmm11,%xmm7
     3e7:	c4 c3 01 21 c3 10    	vinsertps $0x10,%xmm11,%xmm15,%xmm0
     3ed:	c5 78 28 54 24 40    	vmovaps 0x40(%rsp),%xmm10
     3f3:	c5 a8 5c c8          	vsubps %xmm0,%xmm10,%xmm1
     3f7:	c5 78 28 ac 24 60 01 	vmovaps 0x160(%rsp),%xmm13
     3fe:	00 00 
     400:	c5 92 5c 5f 28       	vsubss 0x28(%rdi),%xmm13,%xmm3
     405:	c5 f8 28 74 24 50    	vmovaps 0x50(%rsp),%xmm6
     40b:	c5 90 5c c6          	vsubps %xmm6,%xmm13,%xmm0
     40f:	c5 fa 12 d0          	vmovsldup %xmm0,%xmm2
     413:	c4 e3 69 0c d3 01    	vblendps $0x1,%xmm3,%xmm2,%xmm2
     419:	c5 e8 59 d2          	vmulps %xmm2,%xmm2,%xmm2
     41d:	c5 f0 59 e1          	vmulps %xmm1,%xmm1,%xmm4
     421:	c5 e8 58 e4          	vaddps %xmm4,%xmm2,%xmm4
     425:	c4 c1 7a 16 d5       	vmovshdup %xmm13,%xmm2
     42a:	c5 ea 5c 57 2c       	vsubss 0x2c(%rdi),%xmm2,%xmm2
     42f:	c4 63 79 0c c2 01    	vblendps $0x1,%xmm2,%xmm0,%xmm8
     435:	c4 41 38 59 c0       	vmulps %xmm8,%xmm8,%xmm8
     43a:	c5 b8 58 e4          	vaddps %xmm4,%xmm8,%xmm4
     43e:	c5 58 59 c4          	vmulps %xmm4,%xmm4,%xmm8
     442:	c5 b8 59 e4          	vmulps %xmm4,%xmm8,%xmm4
     446:	c5 78 52 c4          	vrsqrtps %xmm4,%xmm8
     44a:	c5 b8 59 e4          	vmulps %xmm4,%xmm8,%xmm4
     44e:	c5 b8 59 e4          	vmulps %xmm4,%xmm8,%xmm4
     452:	c5 d8 58 25 00 00 00 	vaddps 0x0(%rip),%xmm4,%xmm4        # 45a <simulate+0x45a>
     459:	00 
     45a:	c5 38 59 05 00 00 00 	vmulps 0x0(%rip),%xmm8,%xmm8        # 462 <simulate+0x462>
     461:	00 
     462:	c5 b8 59 e4          	vmulps %xmm4,%xmm8,%xmm4
     466:	c5 08 59 c4          	vmulps %xmm4,%xmm14,%xmm8
     46a:	c4 41 78 28 fe       	vmovaps %xmm14,%xmm15
     46f:	c5 7a 10 0d 00 00 00 	vmovss 0x0(%rip),%xmm9        # 477 <simulate+0x477>
     476:	00 
     477:	c4 41 3a 59 e1       	vmulss %xmm9,%xmm8,%xmm12
     47c:	c5 1a 59 d9          	vmulss %xmm1,%xmm12,%xmm11
     480:	c5 22 58 5f 3c       	vaddss 0x3c(%rdi),%xmm11,%xmm11
     485:	c5 a2 5c ed          	vsubss %xmm5,%xmm11,%xmm5
     489:	c5 fa 11 2c 24       	vmovss %xmm5,(%rsp)
     48e:	c5 d8 59 2d 00 00 00 	vmulps 0x0(%rip),%xmm4,%xmm5        # 496 <simulate+0x496>
     495:	00 
     496:	c5 fa 16 e5          	vmovshdup %xmm5,%xmm4
     49a:	c5 da 59 e0          	vmulss %xmm0,%xmm4,%xmm4
     49e:	c5 52 59 db          	vmulss %xmm3,%xmm5,%xmm11
     4a2:	c5 a2 58 e4          	vaddss %xmm4,%xmm11,%xmm4
     4a6:	c5 7a 16 d8          	vmovshdup %xmm0,%xmm11
     4aa:	c5 20 14 d9          	vunpcklps %xmm1,%xmm11,%xmm11
     4ae:	c4 e3 21 21 db 20    	vinsertps $0x20,%xmm3,%xmm11,%xmm3
     4b4:	c4 63 71 0c da 01    	vblendps $0x1,%xmm2,%xmm1,%xmm11
     4ba:	c5 20 59 dd          	vmulps %xmm5,%xmm11,%xmm11
     4be:	c4 c3 61 21 dc 30    	vinsertps $0x30,%xmm12,%xmm3,%xmm3
     4c4:	c4 c1 50 c6 ec c1    	vshufps $0xc1,%xmm12,%xmm5,%xmm5
     4ca:	c4 e3 51 21 d2 30    	vinsertps $0x30,%xmm2,%xmm5,%xmm2
     4d0:	c5 e0 59 d2          	vmulps %xmm2,%xmm3,%xmm2
     4d4:	c5 a0 16 5f 34       	vmovhps 0x34(%rdi),%xmm11,%xmm3
     4d9:	c5 e0 58 da          	vaddps %xmm2,%xmm3,%xmm3
     4dd:	c4 c1 7a 16 d0       	vmovshdup %xmm8,%xmm2
     4e2:	c5 b2 59 d2          	vmulss %xmm2,%xmm9,%xmm2
     4e6:	c5 fa 12 ea          	vmovsldup %xmm2,%xmm5
     4ea:	c5 d0 59 c0          	vmulps %xmm0,%xmm5,%xmm0
     4ee:	c5 78 58 b4 24 c0 00 	vaddps 0xc0(%rsp),%xmm0,%xmm14
     4f5:	00 00 
     4f7:	c5 fa 16 c1          	vmovshdup %xmm1,%xmm0
     4fb:	c5 ea 59 c0          	vmulss %xmm0,%xmm2,%xmm0
     4ff:	c5 fa 58 47 54       	vaddss 0x54(%rdi),%xmm0,%xmm0
     504:	c5 fa 58 8c 24 d0 00 	vaddss 0xd0(%rsp),%xmm0,%xmm1
     50b:	00 00 
     50d:	c5 f8 28 6c 24 10    	vmovaps 0x10(%rsp),%xmm5
     513:	c5 fa 16 c5          	vmovshdup %xmm5,%xmm0
     517:	c4 e3 79 21 44 24 70 	vinsertps $0x10,0x70(%rsp),%xmm0,%xmm0
     51e:	10 
     51f:	c5 f9 14 44 24 20    	vunpcklpd 0x20(%rsp),%xmm0,%xmm0
     525:	c4 c3 29 21 d5 4c    	vinsertps $0x4c,%xmm13,%xmm10,%xmm2
     52b:	c5 e8 16 d6          	vmovlhps %xmm6,%xmm2,%xmm2
     52f:	c5 e8 5c d0          	vsubps %xmm0,%xmm2,%xmm2
     533:	c5 90 5c c5          	vsubps %xmm5,%xmm13,%xmm0
     537:	c5 f8 59 e8          	vmulps %xmm0,%xmm0,%xmm5
     53b:	c5 68 59 c2          	vmulps %xmm2,%xmm2,%xmm8
     53f:	c4 c3 51 21 e8 90    	vinsertps $0x90,%xmm8,%xmm5,%xmm5
     545:	c4 41 38 c6 c0 ec    	vshufps $0xec,%xmm8,%xmm8,%xmm8
     54b:	c5 b8 58 ed          	vaddps %xmm5,%xmm8,%xmm5
     54f:	c5 42 5c 44 24 60    	vsubss 0x60(%rsp),%xmm7,%xmm8
     555:	c5 7a 16 ca          	vmovshdup %xmm2,%xmm9
     559:	c4 43 31 21 c8 10    	vinsertps $0x10,%xmm8,%xmm9,%xmm9
     55f:	c4 41 30 59 c9       	vmulps %xmm9,%xmm9,%xmm9
     564:	c5 b0 58 ed          	vaddps %xmm5,%xmm9,%xmm5
     568:	c5 50 59 cd          	vmulps %xmm5,%xmm5,%xmm9
     56c:	c5 b0 59 ed          	vmulps %xmm5,%xmm9,%xmm5
     570:	c5 78 52 cd          	vrsqrtps %xmm5,%xmm9
     574:	c5 b0 59 ed          	vmulps %xmm5,%xmm9,%xmm5
     578:	c5 b0 59 ed          	vmulps %xmm5,%xmm9,%xmm5
     57c:	c5 d0 58 2d 00 00 00 	vaddps 0x0(%rip),%xmm5,%xmm5        # 584 <simulate+0x584>
     583:	00 
     584:	c5 30 59 0d 00 00 00 	vmulps 0x0(%rip),%xmm9,%xmm9        # 58c <simulate+0x58c>
     58b:	00 
     58c:	c5 b0 59 ed          	vmulps %xmm5,%xmm9,%xmm5
     590:	c4 41 78 28 d7       	vmovaps %xmm15,%xmm10
     595:	c5 00 59 cd          	vmulps %xmm5,%xmm15,%xmm9
     599:	c4 41 7a 16 d9       	vmovshdup %xmm9,%xmm11
     59e:	c5 22 59 1d 00 00 00 	vmulss 0x0(%rip),%xmm11,%xmm11        # 5a6 <simulate+0x5a6>
     5a5:	00 
     5a6:	c4 41 7a 12 e3       	vmovsldup %xmm11,%xmm12
     5ab:	c5 e9 c6 fa 01       	vshufpd $0x1,%xmm2,%xmm2,%xmm7
     5b0:	c5 98 59 ff          	vmulps %xmm7,%xmm12,%xmm7
     5b4:	c5 c0 58 bc 24 20 01 	vaddps 0x120(%rsp),%xmm7,%xmm7
     5bb:	00 00 
     5bd:	c5 08 5c e7          	vsubps %xmm7,%xmm14,%xmm12
     5c1:	c4 c1 22 59 f0       	vmulss %xmm8,%xmm11,%xmm6
     5c6:	c5 ca 58 b4 24 30 01 	vaddss 0x130(%rsp),%xmm6,%xmm6
     5cd:	00 00 
     5cf:	c5 72 5c fe          	vsubss %xmm6,%xmm1,%xmm15
     5d3:	c5 d0 59 0d 00 00 00 	vmulps 0x0(%rip),%xmm5,%xmm1        # 5db <simulate+0x5db>
     5da:	00 
     5db:	c5 fa 16 e9          	vmovshdup %xmm1,%xmm5
     5df:	c5 ba 59 ed          	vmulss %xmm5,%xmm8,%xmm5
     5e3:	c5 d2 58 6c 24 0c    	vaddss 0xc(%rsp),%xmm5,%xmm5
     5e9:	c5 52 58 84 24 a0 00 	vaddss 0xa0(%rsp),%xmm5,%xmm8
     5f0:	00 00 
     5f2:	c5 b2 59 2d 00 00 00 	vmulss 0x0(%rip),%xmm9,%xmm5        # 5fa <simulate+0x5fa>
     5f9:	00 
     5fa:	c5 d2 59 f0          	vmulss %xmm0,%xmm5,%xmm6
     5fe:	c5 ca 58 b4 24 f0 00 	vaddss 0xf0(%rsp),%xmm6,%xmm6
     605:	00 00 
     607:	c5 da 58 f6          	vaddss %xmm6,%xmm4,%xmm6
     60b:	c5 f2 59 c0          	vmulss %xmm0,%xmm1,%xmm0
     60f:	c5 fa 58 47 64       	vaddss 0x64(%rdi),%xmm0,%xmm0
     614:	c5 fa 58 44 24 04    	vaddss 0x4(%rsp),%xmm0,%xmm0
     61a:	c5 f0 c6 c9 50       	vshufps $0x50,%xmm1,%xmm1,%xmm1
     61f:	c5 f0 59 ca          	vmulps %xmm2,%xmm1,%xmm1
     623:	c5 f0 58 8c 24 50 01 	vaddps 0x150(%rsp),%xmm1,%xmm1
     62a:	00 00 
     62c:	c5 f0 58 a4 24 b0 00 	vaddps 0xb0(%rsp),%xmm1,%xmm4
     633:	00 00 
     635:	c5 fa 10 4f 1c       	vmovss 0x1c(%rdi),%xmm1
     63a:	c5 72 5c f6          	vsubss %xmm6,%xmm1,%xmm14
     63e:	c4 e3 69 21 cd 10    	vinsertps $0x10,%xmm5,%xmm2,%xmm1
     644:	c5 f1 14 8c 24 10 01 	vunpcklpd 0x110(%rsp),%xmm1,%xmm1
     64b:	00 00 
     64d:	c4 e3 69 0c d5 01    	vblendps $0x1,%xmm5,%xmm2,%xmm2
     653:	c5 e9 14 94 24 80 00 	vunpcklpd 0x80(%rsp),%xmm2,%xmm2
     65a:	00 00 
     65c:	c5 f0 59 ea          	vmulps %xmm2,%xmm1,%xmm5
     660:	c5 f0 58 ca          	vaddps %xmm2,%xmm1,%xmm1
     664:	c5 7a 5c 4c 24 08    	vsubss 0x8(%rsp),%xmm0,%xmm9
     66a:	c4 e3 71 0c c5 03    	vblendps $0x3,%xmm5,%xmm1,%xmm0
     670:	c5 d0 58 8c 24 00 01 	vaddps 0x100(%rsp),%xmm5,%xmm1
     677:	00 00 
     679:	c5 e0 5c e8          	vsubps %xmm0,%xmm3,%xmm5
     67d:	c5 e0 58 c1          	vaddps %xmm1,%xmm3,%xmm0
     681:	c4 62 79 18 1d 00 00 	vbroadcastss 0x0(%rip),%xmm11        # 68a <simulate+0x68a>
     688:	00 00 
     68a:	c5 a0 59 cd          	vmulps %xmm5,%xmm11,%xmm1
     68e:	c4 e3 71 0c c0 03    	vblendps $0x3,%xmm0,%xmm1,%xmm0
     694:	c5 f8 10 4f 20       	vmovups 0x20(%rdi),%xmm1
     699:	c5 f0 5c d0          	vsubps %xmm0,%xmm1,%xmm2
     69d:	c5 f0 58 f8          	vaddps %xmm0,%xmm1,%xmm7
     6a1:	c5 fa 12 ca          	vmovsldup %xmm2,%xmm1
     6a5:	c5 78 29 b4 24 f0 00 	vmovaps %xmm14,0xf0(%rsp)
     6ac:	00 00 
     6ae:	c4 c3 71 0c ce 01    	vblendps $0x1,%xmm14,%xmm1,%xmm1
     6b4:	c5 a8 59 c9          	vmulps %xmm1,%xmm10,%xmm1
     6b8:	c5 90 58 c9          	vaddps %xmm1,%xmm13,%xmm1
     6bc:	c5 fa 16 f2          	vmovshdup %xmm2,%xmm6
     6c0:	c5 f8 29 b4 24 20 01 	vmovaps %xmm6,0x120(%rsp)
     6c7:	00 00 
     6c9:	c5 f8 28 c2          	vmovaps %xmm2,%xmm0
     6cd:	c5 f8 29 94 24 a0 00 	vmovaps %xmm2,0xa0(%rsp)
     6d4:	00 00 
     6d6:	c5 fa 10 1d 00 00 00 	vmovss 0x0(%rip),%xmm3        # 6de <simulate+0x6de>
     6dd:	00 
     6de:	c5 ca 59 d3          	vmulss %xmm3,%xmm6,%xmm2
     6e2:	c5 ea 58 74 24 40    	vaddss 0x40(%rsp),%xmm2,%xmm6
     6e8:	48 8b 47 08          	mov    0x8(%rdi),%rax
     6ec:	c5 7a 11 77 1c       	vmovss %xmm14,0x1c(%rdi)
     6f1:	c5 d1 c6 d5 01       	vshufpd $0x1,%xmm5,%xmm5,%xmm2
     6f6:	c5 f9 29 94 24 10 01 	vmovapd %xmm2,0x110(%rsp)
     6fd:	00 00 
     6ff:	c5 fa 11 57 34       	vmovss %xmm2,0x34(%rdi)
     704:	c5 d0 c6 d5 ff       	vshufps $0xff,%xmm5,%xmm5,%xmm2
     709:	c5 f8 29 94 24 00 01 	vmovaps %xmm2,0x100(%rsp)
     710:	00 00 
     712:	c5 fa 11 57 38       	vmovss %xmm2,0x38(%rdi)
     717:	c5 7a 10 34 24       	vmovss (%rsp),%xmm14
     71c:	c5 7a 11 77 3c       	vmovss %xmm14,0x3c(%rdi)
     721:	c5 78 29 a4 24 d0 00 	vmovaps %xmm12,0xd0(%rsp)
     728:	00 00 
     72a:	c5 78 13 67 4c       	vmovlps %xmm12,0x4c(%rdi)
     72f:	c5 78 29 fa          	vmovaps %xmm15,%xmm2
     733:	c5 7a 11 bc 24 c0 00 	vmovss %xmm15,0xc0(%rsp)
     73a:	00 00 
     73c:	c5 7a 11 7f 54       	vmovss %xmm15,0x54(%rdi)
     741:	c5 7a 11 4f 64       	vmovss %xmm9,0x64(%rdi)
     746:	c4 41 78 28 e9       	vmovaps %xmm9,%xmm13
     74b:	c5 78 29 8c 24 b0 00 	vmovaps %xmm9,0xb0(%rsp)
     752:	00 00 
     754:	c5 d9 c6 ec 01       	vshufpd $0x1,%xmm4,%xmm4,%xmm5
     759:	c5 f9 29 6c 24 40    	vmovapd %xmm5,0x40(%rsp)
     75f:	c5 fa 11 6f 7c       	vmovss %xmm5,0x7c(%rdi)
     764:	c5 d8 c6 ec ff       	vshufps $0xff,%xmm4,%xmm4,%xmm5
     769:	c5 f8 29 ac 24 30 01 	vmovaps %xmm5,0x130(%rsp)
     770:	00 00 
     772:	c5 fa 11 af 80 00 00 	vmovss %xmm5,0x80(%rdi)
     779:	00 
     77a:	c5 7a 11 44 24 0c    	vmovss %xmm8,0xc(%rsp)
     780:	c5 7a 11 87 84 00 00 	vmovss %xmm8,0x84(%rdi)
     787:	00 
     788:	c5 f8 13 4f 10       	vmovlps %xmm1,0x10(%rdi)
     78d:	c5 fa 11 77 18       	vmovss %xmm6,0x18(%rdi)
     792:	c4 e3 41 0c e8 03    	vblendps $0x3,%xmm0,%xmm7,%xmm5
     798:	c5 f8 11 6f 20       	vmovups %xmm5,0x20(%rdi)
     79d:	c5 8a 59 eb          	vmulss %xmm3,%xmm14,%xmm5
     7a1:	c5 52 58 bc 24 90 00 	vaddss 0x90(%rsp),%xmm5,%xmm15
     7a8:	00 00 
     7aa:	c4 c1 18 59 ea       	vmulps %xmm10,%xmm12,%xmm5
     7af:	c5 50 58 4c 24 50    	vaddps 0x50(%rsp),%xmm5,%xmm9
     7b5:	c5 ea 59 eb          	vmulss %xmm3,%xmm2,%xmm5
     7b9:	c5 52 58 64 24 30    	vaddss 0x30(%rsp),%xmm5,%xmm12
     7bf:	c5 d8 58 ac 24 40 01 	vaddps 0x140(%rsp),%xmm4,%xmm5
     7c6:	00 00 
     7c8:	c5 a0 59 e4          	vmulps %xmm4,%xmm11,%xmm4
     7cc:	c4 e3 59 0c e5 03    	vblendps $0x3,%xmm5,%xmm4,%xmm4
     7d2:	c5 f8 28 84 24 e0 00 	vmovaps 0xe0(%rsp),%xmm0
     7d9:	00 00 
     7db:	c5 f9 14 6c 24 20    	vunpcklpd 0x20(%rsp),%xmm0,%xmm5
     7e1:	c5 d8 5c c5          	vsubps %xmm5,%xmm4,%xmm0
     7e5:	c5 58 58 f5          	vaddps %xmm5,%xmm4,%xmm14
     7e9:	c5 f8 29 44 24 20    	vmovaps %xmm0,0x20(%rsp)
     7ef:	c5 fa 12 e0          	vmovsldup %xmm0,%xmm4
     7f3:	c4 c3 59 0c e5 01    	vblendps $0x1,%xmm13,%xmm4,%xmm4
     7f9:	c5 a8 59 e4          	vmulps %xmm4,%xmm10,%xmm4
     7fd:	c5 58 58 54 24 10    	vaddps 0x10(%rsp),%xmm4,%xmm10
     803:	c5 fa 16 d0          	vmovshdup %xmm0,%xmm2
     807:	c5 f8 29 54 24 10    	vmovaps %xmm2,0x10(%rsp)
     80d:	c5 ea 59 e3          	vmulss %xmm3,%xmm2,%xmm4
     811:	c5 da 58 54 24 70    	vaddss 0x70(%rsp),%xmm4,%xmm2
     817:	c5 fa 11 54 24 08    	vmovss %xmm2,0x8(%rsp)
     81d:	c5 7a 11 bc 24 80 00 	vmovss %xmm15,0x80(%rsp)
     824:	00 00 
     826:	c5 7a 11 7f 30       	vmovss %xmm15,0x30(%rdi)
     82b:	c5 78 13 4f 40       	vmovlps %xmm9,0x40(%rdi)
     830:	c5 7a 11 67 48       	vmovss %xmm12,0x48(%rdi)
     835:	c5 7a 11 a4 24 90 00 	vmovss %xmm12,0x90(%rsp)
     83c:	00 00 
     83e:	c5 78 13 57 58       	vmovlps %xmm10,0x58(%rdi)
     843:	c5 fa 11 57 60       	vmovss %xmm2,0x60(%rdi)
     848:	c4 e3 09 0c e0 03    	vblendps $0x3,%xmm0,%xmm14,%xmm4
     84e:	c5 f8 11 67 68       	vmovups %xmm4,0x68(%rdi)
     853:	c5 ba 59 db          	vmulss %xmm3,%xmm8,%xmm3
     857:	c5 e2 58 44 24 60    	vaddss 0x60(%rsp),%xmm3,%xmm0
     85d:	c5 fa 11 44 24 04    	vmovss %xmm0,0x4(%rsp)
     863:	c5 41 c6 c7 01       	vshufpd $0x1,%xmm7,%xmm7,%xmm8
     868:	c4 c1 72 5c d8       	vsubss %xmm8,%xmm1,%xmm3
     86d:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     871:	c4 c1 4a 5c e7       	vsubss %xmm15,%xmm6,%xmm4
     876:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
     87a:	c5 e2 58 dc          	vaddss %xmm4,%xmm3,%xmm3
     87e:	c5 c0 c6 e7 ff       	vshufps $0xff,%xmm7,%xmm7,%xmm4
     883:	c5 fa 16 c1          	vmovshdup %xmm1,%xmm0
     887:	c5 fa 5c ec          	vsubss %xmm4,%xmm0,%xmm5
     88b:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     88f:	c5 d2 58 db          	vaddss %xmm3,%xmm5,%xmm3
     893:	c5 e2 52 eb          	vrsqrtss %xmm3,%xmm3,%xmm5
     897:	c5 e2 59 dd          	vmulss %xmm5,%xmm3,%xmm3
     89b:	c5 e2 59 dd          	vmulss %xmm5,%xmm3,%xmm3
     89f:	c5 7a 10 1d 00 00 00 	vmovss 0x0(%rip),%xmm11        # 8a7 <simulate+0x8a7>
     8a6:	00 
     8a7:	c5 a2 58 db          	vaddss %xmm3,%xmm11,%xmm3
     8ab:	c5 7a 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm13        # 8b3 <simulate+0x8b3>
     8b2:	00 
     8b3:	c5 92 59 ed          	vmulss %xmm5,%xmm13,%xmm5
     8b7:	c5 d2 59 db          	vmulss %xmm3,%xmm5,%xmm3
     8bb:	c5 fa 11 5c 24 70    	vmovss %xmm3,0x70(%rsp)
     8c1:	c4 c1 72 5c d9       	vsubss %xmm9,%xmm1,%xmm3
     8c6:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     8ca:	c4 c1 4a 5c ec       	vsubss %xmm12,%xmm6,%xmm5
     8cf:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     8d3:	c5 e2 58 dd          	vaddss %xmm5,%xmm3,%xmm3
     8d7:	c4 c1 7a 16 f9       	vmovshdup %xmm9,%xmm7
     8dc:	c5 fa 5c ef          	vsubss %xmm7,%xmm0,%xmm5
     8e0:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     8e4:	c5 d2 58 db          	vaddss %xmm3,%xmm5,%xmm3
     8e8:	c5 e2 52 eb          	vrsqrtss %xmm3,%xmm3,%xmm5
     8ec:	c5 e2 59 dd          	vmulss %xmm5,%xmm3,%xmm3
     8f0:	c5 e2 59 dd          	vmulss %xmm5,%xmm3,%xmm3
     8f4:	c5 a2 58 db          	vaddss %xmm3,%xmm11,%xmm3
     8f8:	c5 92 59 ed          	vmulss %xmm5,%xmm13,%xmm5
     8fc:	c4 41 78 28 fd       	vmovaps %xmm13,%xmm15
     901:	c5 d2 59 d3          	vmulss %xmm3,%xmm5,%xmm2
     905:	c5 fa 11 54 24 60    	vmovss %xmm2,0x60(%rsp)
     90b:	c4 c1 72 5c da       	vsubss %xmm10,%xmm1,%xmm3
     910:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     914:	c5 7a 10 64 24 08    	vmovss 0x8(%rsp),%xmm12
     91a:	c4 c1 4a 5c ec       	vsubss %xmm12,%xmm6,%xmm5
     91f:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     923:	c5 e2 58 dd          	vaddss %xmm5,%xmm3,%xmm3
     927:	c4 c1 7a 16 ea       	vmovshdup %xmm10,%xmm5
     92c:	c5 7a 5c ed          	vsubss %xmm5,%xmm0,%xmm13
     930:	c4 41 12 59 ed       	vmulss %xmm13,%xmm13,%xmm13
     935:	c5 92 58 db          	vaddss %xmm3,%xmm13,%xmm3
     939:	c5 62 52 eb          	vrsqrtss %xmm3,%xmm3,%xmm13
     93d:	c5 92 59 db          	vmulss %xmm3,%xmm13,%xmm3
     941:	c5 92 59 db          	vmulss %xmm3,%xmm13,%xmm3
     945:	c5 a2 58 db          	vaddss %xmm3,%xmm11,%xmm3
     949:	c4 41 12 59 ef       	vmulss %xmm15,%xmm13,%xmm13
     94e:	c5 92 59 d3          	vmulss %xmm3,%xmm13,%xmm2
     952:	c5 fa 11 54 24 50    	vmovss %xmm2,0x50(%rsp)
     958:	c4 c1 09 c6 de 01    	vshufpd $0x1,%xmm14,%xmm14,%xmm3
     95e:	c5 f2 5c cb          	vsubss %xmm3,%xmm1,%xmm1
     962:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
     966:	c5 7a 10 7c 24 04    	vmovss 0x4(%rsp),%xmm15
     96c:	c4 c1 4a 5c d7       	vsubss %xmm15,%xmm6,%xmm2
     971:	c5 ea 59 d2          	vmulss %xmm2,%xmm2,%xmm2
     975:	c5 f2 58 ca          	vaddss %xmm2,%xmm1,%xmm1
     979:	c4 c1 08 c6 d6 ff    	vshufps $0xff,%xmm14,%xmm14,%xmm2
     97f:	c5 fa 5c c2          	vsubss %xmm2,%xmm0,%xmm0
     983:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
     987:	c5 fa 58 c1          	vaddss %xmm1,%xmm0,%xmm0
     98b:	c5 fa 52 c8          	vrsqrtss %xmm0,%xmm0,%xmm1
     98f:	c5 fa 59 c1          	vmulss %xmm1,%xmm0,%xmm0
     993:	c5 fa 59 c1          	vmulss %xmm1,%xmm0,%xmm0
     997:	c4 41 78 28 eb       	vmovaps %xmm11,%xmm13
     99c:	c5 a2 58 c0          	vaddss %xmm0,%xmm11,%xmm0
     9a0:	c5 7a 10 35 00 00 00 	vmovss 0x0(%rip),%xmm14        # 9a8 <simulate+0x9a8>
     9a7:	00 
     9a8:	c5 8a 59 c9          	vmulss %xmm1,%xmm14,%xmm1
     9ac:	c5 f2 59 c0          	vmulss %xmm0,%xmm1,%xmm0
     9b0:	c5 fa 11 44 24 30    	vmovss %xmm0,0x30(%rsp)
     9b6:	c4 c1 3a 5c c1       	vsubss %xmm9,%xmm8,%xmm0
     9bb:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
     9bf:	c5 da 5c cf          	vsubss %xmm7,%xmm4,%xmm1
     9c3:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
     9c7:	c5 f2 58 c0          	vaddss %xmm0,%xmm1,%xmm0
     9cb:	c5 fa 10 b4 24 90 00 	vmovss 0x90(%rsp),%xmm6
     9d2:	00 00 
     9d4:	c5 7a 10 9c 24 80 00 	vmovss 0x80(%rsp),%xmm11
     9db:	00 00 
     9dd:	c5 a2 5c ce          	vsubss %xmm6,%xmm11,%xmm1
     9e1:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
     9e5:	c5 fa 58 c1          	vaddss %xmm1,%xmm0,%xmm0
     9e9:	c5 fa 52 c8          	vrsqrtss %xmm0,%xmm0,%xmm1
     9ed:	c5 fa 59 c1          	vmulss %xmm1,%xmm0,%xmm0
     9f1:	c5 fa 59 c1          	vmulss %xmm1,%xmm0,%xmm0
     9f5:	c5 92 58 c0          	vaddss %xmm0,%xmm13,%xmm0
     9f9:	c5 8a 59 c9          	vmulss %xmm1,%xmm14,%xmm1
     9fd:	c5 f2 59 c0          	vmulss %xmm0,%xmm1,%xmm0
     a01:	c5 fa 11 84 24 40 01 	vmovss %xmm0,0x140(%rsp)
     a08:	00 00 
     a0a:	c4 c1 3a 5c c2       	vsubss %xmm10,%xmm8,%xmm0
     a0f:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
     a13:	c4 c1 22 5c cc       	vsubss %xmm12,%xmm11,%xmm1
     a18:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
     a1c:	c5 fa 58 c1          	vaddss %xmm1,%xmm0,%xmm0
     a20:	c5 da 5c cd          	vsubss %xmm5,%xmm4,%xmm1
     a24:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
     a28:	c5 f2 58 c0          	vaddss %xmm0,%xmm1,%xmm0
     a2c:	c5 fa 52 c8          	vrsqrtss %xmm0,%xmm0,%xmm1
     a30:	c5 fa 59 c1          	vmulss %xmm1,%xmm0,%xmm0
     a34:	c5 fa 59 c1          	vmulss %xmm1,%xmm0,%xmm0
     a38:	c5 92 58 c0          	vaddss %xmm0,%xmm13,%xmm0
     a3c:	c5 8a 59 c9          	vmulss %xmm1,%xmm14,%xmm1
     a40:	c5 f2 59 c0          	vmulss %xmm0,%xmm1,%xmm0
     a44:	c5 fa 11 84 24 e0 00 	vmovss %xmm0,0xe0(%rsp)
     a4b:	00 00 
     a4d:	c5 ba 5c c3          	vsubss %xmm3,%xmm8,%xmm0
     a51:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
     a55:	c5 da 5c e2          	vsubss %xmm2,%xmm4,%xmm4
     a59:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
     a5d:	c5 da 58 c0          	vaddss %xmm0,%xmm4,%xmm0
     a61:	c4 c1 22 5c e7       	vsubss %xmm15,%xmm11,%xmm4
     a66:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
     a6a:	c5 fa 58 c4          	vaddss %xmm4,%xmm0,%xmm0
     a6e:	c5 fa 52 e0          	vrsqrtss %xmm0,%xmm0,%xmm4
     a72:	c5 fa 59 c4          	vmulss %xmm4,%xmm0,%xmm0
     a76:	c5 fa 59 c4          	vmulss %xmm4,%xmm0,%xmm0
     a7a:	c5 92 58 c0          	vaddss %xmm0,%xmm13,%xmm0
     a7e:	c5 8a 59 e4          	vmulss %xmm4,%xmm14,%xmm4
     a82:	c5 5a 59 d8          	vmulss %xmm0,%xmm4,%xmm11
     a86:	c4 c1 32 5c c2       	vsubss %xmm10,%xmm9,%xmm0
     a8b:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
     a8f:	c4 c1 4a 5c e4       	vsubss %xmm12,%xmm6,%xmm4
     a94:	c5 78 28 c6          	vmovaps %xmm6,%xmm8
     a98:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
     a9c:	c5 fa 58 c4          	vaddss %xmm4,%xmm0,%xmm0
     aa0:	c5 c2 5c e5          	vsubss %xmm5,%xmm7,%xmm4
     aa4:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
     aa8:	c5 da 58 c0          	vaddss %xmm0,%xmm4,%xmm0
     aac:	c5 fa 52 e0          	vrsqrtss %xmm0,%xmm0,%xmm4
     ab0:	c5 fa 59 c4          	vmulss %xmm4,%xmm0,%xmm0
     ab4:	c5 fa 59 c4          	vmulss %xmm4,%xmm0,%xmm0
     ab8:	c5 92 58 c0          	vaddss %xmm0,%xmm13,%xmm0
     abc:	c5 8a 59 e4          	vmulss %xmm4,%xmm14,%xmm4
     ac0:	c5 da 59 e0          	vmulss %xmm0,%xmm4,%xmm4
     ac4:	c5 b2 5c c3          	vsubss %xmm3,%xmm9,%xmm0
     ac8:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
     acc:	c5 c2 5c f2          	vsubss %xmm2,%xmm7,%xmm6
     ad0:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
     ad4:	c5 ca 58 c0          	vaddss %xmm0,%xmm6,%xmm0
     ad8:	c4 c1 3a 5c f7       	vsubss %xmm15,%xmm8,%xmm6
     add:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
     ae1:	c5 fa 58 c6          	vaddss %xmm6,%xmm0,%xmm0
     ae5:	c5 fa 52 f0          	vrsqrtss %xmm0,%xmm0,%xmm6
     ae9:	c5 fa 59 c6          	vmulss %xmm6,%xmm0,%xmm0
     aed:	c5 fa 59 c6          	vmulss %xmm6,%xmm0,%xmm0
     af1:	c5 92 58 c0          	vaddss %xmm0,%xmm13,%xmm0
     af5:	c5 8a 59 f6          	vmulss %xmm6,%xmm14,%xmm6
     af9:	c5 ca 59 f0          	vmulss %xmm0,%xmm6,%xmm6
     afd:	c5 aa 5c c3          	vsubss %xmm3,%xmm10,%xmm0
     b01:	c5 d2 5c d2          	vsubss %xmm2,%xmm5,%xmm2
     b05:	c5 7a 11 7f 78       	vmovss %xmm15,0x78(%rdi)
     b0a:	c4 c1 1a 5c df       	vsubss %xmm15,%xmm12,%xmm3
     b0f:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
     b13:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     b17:	c5 fa 58 c3          	vaddss %xmm3,%xmm0,%xmm0
     b1b:	c5 ea 59 d2          	vmulss %xmm2,%xmm2,%xmm2
     b1f:	c5 ea 58 c0          	vaddss %xmm0,%xmm2,%xmm0
     b23:	c5 fa 52 d0          	vrsqrtss %xmm0,%xmm0,%xmm2
     b27:	c5 fa 59 c2          	vmulss %xmm2,%xmm0,%xmm0
     b2b:	c5 fa 59 c2          	vmulss %xmm2,%xmm0,%xmm0
     b2f:	c5 92 58 c0          	vaddss %xmm0,%xmm13,%xmm0
     b33:	c5 8a 59 d2          	vmulss %xmm2,%xmm14,%xmm2
     b37:	c5 ea 59 c0          	vmulss %xmm0,%xmm2,%xmm0
     b3b:	c5 f8 28 94 24 f0 00 	vmovaps 0xf0(%rsp),%xmm2
     b42:	00 00 
     b44:	c5 ea 59 d2          	vmulss %xmm2,%xmm2,%xmm2
     b48:	c5 f8 28 9c 24 a0 00 	vmovaps 0xa0(%rsp),%xmm3
     b4f:	00 00 
     b51:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     b55:	c5 e2 58 d2          	vaddss %xmm2,%xmm3,%xmm2
     b59:	c5 f8 28 9c 24 20 01 	vmovaps 0x120(%rsp),%xmm3
     b60:	00 00 
     b62:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     b66:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
     b6a:	c5 f8 28 9c 24 10 01 	vmovaps 0x110(%rsp),%xmm3
     b71:	00 00 
     b73:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     b77:	c5 f8 28 ac 24 00 01 	vmovaps 0x100(%rsp),%xmm5
     b7e:	00 00 
     b80:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     b84:	c5 d2 58 db          	vaddss %xmm3,%xmm5,%xmm3
     b88:	c5 fa 10 2c 24       	vmovss (%rsp),%xmm5
     b8d:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     b91:	c5 e2 58 dd          	vaddss %xmm5,%xmm3,%xmm3
     b95:	c5 f8 28 ac 24 d0 00 	vmovaps 0xd0(%rsp),%xmm5
     b9c:	00 00 
     b9e:	c5 d0 59 ed          	vmulps %xmm5,%xmm5,%xmm5
     ba2:	c5 7a 16 c5          	vmovshdup %xmm5,%xmm8
     ba6:	c5 ba 58 ed          	vaddss %xmm5,%xmm8,%xmm5
     baa:	c5 fa 10 bc 24 c0 00 	vmovss 0xc0(%rsp),%xmm7
     bb1:	00 00 
     bb3:	c5 42 59 c7          	vmulss %xmm7,%xmm7,%xmm8
     bb7:	c5 ba 58 ed          	vaddss %xmm5,%xmm8,%xmm5
     bbb:	c5 e2 59 1d 00 00 00 	vmulss 0x0(%rip),%xmm3,%xmm3        # bc3 <simulate+0xbc3>
     bc2:	00 
     bc3:	c5 d2 59 2d 00 00 00 	vmulss 0x0(%rip),%xmm5,%xmm5        # bcb <simulate+0xbcb>
     bca:	00 
     bcb:	c5 e2 58 dd          	vaddss %xmm5,%xmm3,%xmm3
     bcf:	c5 f8 28 ac 24 b0 00 	vmovaps 0xb0(%rsp),%xmm5
     bd6:	00 00 
     bd8:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     bdc:	c5 f8 28 7c 24 20    	vmovaps 0x20(%rsp),%xmm7
     be2:	c5 42 59 c7          	vmulss %xmm7,%xmm7,%xmm8
     be6:	c5 ba 58 ed          	vaddss %xmm5,%xmm8,%xmm5
     bea:	c5 f8 28 7c 24 10    	vmovaps 0x10(%rsp),%xmm7
     bf0:	c5 42 59 c7          	vmulss %xmm7,%xmm7,%xmm8
     bf4:	c5 ba 58 ed          	vaddss %xmm5,%xmm8,%xmm5
     bf8:	c5 d2 59 2d 00 00 00 	vmulss 0x0(%rip),%xmm5,%xmm5        # c00 <simulate+0xc00>
     bff:	00 
     c00:	c5 e2 58 dd          	vaddss %xmm5,%xmm3,%xmm3
     c04:	c5 ea 59 15 00 00 00 	vmulss 0x0(%rip),%xmm2,%xmm2        # c0c <simulate+0xc0c>
     c0b:	00 
     c0c:	c5 e2 58 d2          	vaddss %xmm2,%xmm3,%xmm2
     c10:	c5 f8 28 5c 24 40    	vmovaps 0x40(%rsp),%xmm3
     c16:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     c1a:	c5 f8 28 ac 24 30 01 	vmovaps 0x130(%rsp),%xmm5
     c21:	00 00 
     c23:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     c27:	c5 d2 58 db          	vaddss %xmm3,%xmm5,%xmm3
     c2b:	c5 fa 10 6c 24 0c    	vmovss 0xc(%rsp),%xmm5
     c31:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     c35:	c5 fa 10 8c 24 e0 00 	vmovss 0xe0(%rsp),%xmm1
     c3c:	00 00 
     c3e:	c5 f2 59 0d 00 00 00 	vmulss 0x0(%rip),%xmm1,%xmm1        # c46 <simulate+0xc46>
     c45:	00 
     c46:	c5 fa 10 bc 24 40 01 	vmovss 0x140(%rsp),%xmm7
     c4d:	00 00 
     c4f:	c5 42 59 05 00 00 00 	vmulss 0x0(%rip),%xmm7,%xmm8        # c57 <simulate+0xc57>
     c56:	00 
     c57:	c5 e2 58 dd          	vaddss %xmm5,%xmm3,%xmm3
     c5b:	c5 ba 58 c9          	vaddss %xmm1,%xmm8,%xmm1
     c5f:	c5 da 59 25 00 00 00 	vmulss 0x0(%rip),%xmm4,%xmm4        # c67 <simulate+0xc67>
     c66:	00 
     c67:	c5 f2 58 cc          	vaddss %xmm4,%xmm1,%xmm1
     c6b:	c5 e2 59 1d 00 00 00 	vmulss 0x0(%rip),%xmm3,%xmm3        # c73 <simulate+0xc73>
     c72:	00 
     c73:	c5 f2 58 cb          	vaddss %xmm3,%xmm1,%xmm1
     c77:	c5 fa 10 5c 24 50    	vmovss 0x50(%rsp),%xmm3
     c7d:	c5 e2 59 1d 00 00 00 	vmulss 0x0(%rip),%xmm3,%xmm3        # c85 <simulate+0xc85>
     c84:	00 
     c85:	c5 fa 10 64 24 70    	vmovss 0x70(%rsp),%xmm4
     c8b:	c5 da 59 25 00 00 00 	vmulss 0x0(%rip),%xmm4,%xmm4        # c93 <simulate+0xc93>
     c92:	00 
     c93:	c5 da 58 db          	vaddss %xmm3,%xmm4,%xmm3
     c97:	c5 fa 10 64 24 60    	vmovss 0x60(%rsp),%xmm4
     c9d:	c5 da 59 25 00 00 00 	vmulss 0x0(%rip),%xmm4,%xmm4        # ca5 <simulate+0xca5>
     ca4:	00 
     ca5:	c5 ea 58 d4          	vaddss %xmm4,%xmm2,%xmm2
     ca9:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
     cad:	c5 ca 59 1d 00 00 00 	vmulss 0x0(%rip),%xmm6,%xmm3        # cb5 <simulate+0xcb5>
     cb4:	00 
     cb5:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
     cb9:	c5 a2 59 1d 00 00 00 	vmulss 0x0(%rip),%xmm11,%xmm3        # cc1 <simulate+0xcc1>
     cc0:	00 
     cc1:	c5 fa 10 64 24 30    	vmovss 0x30(%rsp),%xmm4
     cc7:	c5 da 59 25 00 00 00 	vmulss 0x0(%rip),%xmm4,%xmm4        # ccf <simulate+0xccf>
     cce:	00 
     ccf:	c5 f2 58 cc          	vaddss %xmm4,%xmm1,%xmm1
     cd3:	c5 f2 58 cb          	vaddss %xmm3,%xmm1,%xmm1
     cd7:	c5 fa 59 05 00 00 00 	vmulss 0x0(%rip),%xmm0,%xmm0        # cdf <simulate+0xcdf>
     cde:	00 
     cdf:	c5 f2 58 c0          	vaddss %xmm0,%xmm1,%xmm0
     ce3:	c5 ea 58 c0          	vaddss %xmm0,%xmm2,%xmm0
     ce7:	48 ff c0             	inc    %rax
     cea:	48 89 47 08          	mov    %rax,0x8(%rdi)
     cee:	48 b9 a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%rcx
     cf5:	24 03 e5 
     cf8:	48 0f af c8          	imul   %rax,%rcx
     cfc:	48 b8 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%rax
     d03:	6b 00 00 
     d06:	48 01 c8             	add    %rcx,%rax
     d09:	48 0f ac c0 06       	shrd   $0x6,%rax,%rax
     d0e:	48 b9 94 57 53 fe 5a 	movabs $0x35afe535794,%rcx
     d15:	03 00 00 
     d18:	48 39 c8             	cmp    %rcx,%rax
     d1b:	76 16                	jbe    d33 <simulate+0xd33>
     d1d:	48 8b 43 08          	mov    0x8(%rbx),%rax
     d21:	48 3b 03             	cmp    (%rbx),%rax
     d24:	75 2d                	jne    d53 <simulate+0xd53>
     d26:	48 81 c4 70 01 00 00 	add    $0x170,%rsp
     d2d:	5b                   	pop    %rbx
     d2e:	e9 00 00 00 00       	jmp    d33 <simulate+0xd33>
     d33:	c5 fa 11 44 24 10    	vmovss %xmm0,0x10(%rsp)
     d39:	c5 fa 10 44 24 10    	vmovss 0x10(%rsp),%xmm0
     d3f:	e8 00 00 00 00       	call   d44 <simulate+0xd44>
     d44:	c5 fa 10 44 24 10    	vmovss 0x10(%rsp),%xmm0
     d4a:	48 8b 43 08          	mov    0x8(%rbx),%rax
     d4e:	48 3b 03             	cmp    (%rbx),%rax
     d51:	74 d3                	je     d26 <simulate+0xd26>
     d53:	48 81 c4 70 01 00 00 	add    $0x170,%rsp
     d5a:	5b                   	pop    %rbx
     d5b:	c3                   	ret
     d5c:	0f 1f 40 00          	nopl   0x0(%rax)

0000000000000d60 <init_state>:
     d60:	53                   	push   %rbx
     d61:	48 89 fb             	mov    %rdi,%rbx
     d64:	bf 00 00 00 00       	mov    $0x0,%edi
     d69:	e8 00 00 00 00       	call   d6e <init_state+0xe>
     d6e:	48 89 03             	mov    %rax,(%rbx)
     d71:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
     d75:	c5 f8 11 43 18       	vmovups %xmm0,0x18(%rbx)
     d7a:	c5 f8 11 43 08       	vmovups %xmm0,0x8(%rbx)
     d7f:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # d87 <init_state+0x27>
     d86:	00 
     d87:	c5 f8 11 43 28       	vmovups %xmm0,0x28(%rbx)
     d8c:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # d94 <init_state+0x34>
     d93:	00 
     d94:	c5 f8 11 43 38       	vmovups %xmm0,0x38(%rbx)
     d99:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # da1 <init_state+0x41>
     da0:	00 
     da1:	c5 f8 11 43 48       	vmovups %xmm0,0x48(%rbx)
     da6:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # dae <init_state+0x4e>
     dad:	00 
     dae:	c5 f8 11 43 58       	vmovups %xmm0,0x58(%rbx)
     db3:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # dbb <init_state+0x5b>
     dba:	00 
     dbb:	c5 f8 11 43 68       	vmovups %xmm0,0x68(%rbx)
     dc0:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # dc8 <init_state+0x68>
     dc7:	00 
     dc8:	c5 f8 11 43 78       	vmovups %xmm0,0x78(%rbx)
     dcd:	5b                   	pop    %rbx
     dce:	c3                   	ret
     dcf:	90                   	nop

0000000000000dd0 <main>:
     dd0:	41 57                	push   %r15
     dd2:	41 56                	push   %r14
     dd4:	41 55                	push   %r13
     dd6:	41 54                	push   %r12
     dd8:	53                   	push   %rbx
     dd9:	48 81 ec 50 01 00 00 	sub    $0x150,%rsp
     de0:	bf 00 00 00 00       	mov    $0x0,%edi
     de5:	e8 00 00 00 00       	call   dea <main+0x1a>
     dea:	48 89 c3             	mov    %rax,%rbx
     ded:	c5 fc 28 05 00 00 00 	vmovaps 0x0(%rip),%ymm0        # df5 <main+0x25>
     df4:	00 
     df5:	c5 fc 11 84 24 10 01 	vmovups %ymm0,0x110(%rsp)
     dfc:	00 00 
     dfe:	c5 7c 28 3d 00 00 00 	vmovaps 0x0(%rip),%ymm15        # e06 <main+0x36>
     e05:	00 
     e06:	c5 f8 28 2d 00 00 00 	vmovaps 0x0(%rip),%xmm5        # e0e <main+0x3e>
     e0d:	00 
     e0e:	c5 7b 12 0d 00 00 00 	vmovddup 0x0(%rip),%xmm9        # e16 <main+0x46>
     e15:	00 
     e16:	c5 fb 12 05 00 00 00 	vmovddup 0x0(%rip),%xmm0        # e1e <main+0x4e>
     e1d:	00 
     e1e:	c5 f8 29 44 24 30    	vmovaps %xmm0,0x30(%rsp)
     e24:	c5 7b 12 35 00 00 00 	vmovddup 0x0(%rip),%xmm14        # e2c <main+0x5c>
     e2b:	00 
     e2c:	c5 78 28 2d 00 00 00 	vmovaps 0x0(%rip),%xmm13        # e34 <main+0x64>
     e33:	00 
     e34:	45 31 f6             	xor    %r14d,%r14d
     e37:	49 bf a5 46 8d ae 77 	movabs $0xe5032477ae8d46a5,%r15
     e3e:	24 03 e5 
     e41:	49 bc 80 f2 6a ca 5f 	movabs $0x6b5fca6af280,%r12
     e48:	6b 00 00 
     e4b:	49 bd 94 57 53 fe 5a 	movabs $0x35afe535794,%r13
     e52:	03 00 00 
     e55:	eb 12                	jmp    e69 <main+0x99>
     e57:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
     e5e:	00 00 
     e60:	49 39 de             	cmp    %rbx,%r14
     e63:	0f 84 dd 06 00 00    	je     1546 <main+0x776>
     e69:	49 39 de             	cmp    %rbx,%r14
     e6c:	7d f2                	jge    e60 <main+0x90>
     e6e:	c4 c1 00 c6 e7 ff    	vshufps $0xff,%xmm15,%xmm15,%xmm4
     e74:	c5 92 5c c4          	vsubss %xmm4,%xmm13,%xmm0
     e78:	c4 c1 01 c6 ff 01    	vshufpd $0x1,%xmm15,%xmm15,%xmm7
     e7e:	c4 c1 7a 16 d5       	vmovshdup %xmm13,%xmm2
     e83:	c5 ea 5c cf          	vsubss %xmm7,%xmm2,%xmm1
     e87:	c4 41 7a 16 c7       	vmovshdup %xmm15,%xmm8
     e8c:	c4 c1 52 5c d8       	vsubss %xmm8,%xmm5,%xmm3
     e91:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
     e95:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
     e99:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     e9d:	c5 f2 58 cb          	vaddss %xmm3,%xmm1,%xmm1
     ea1:	c5 f2 58 c0          	vaddss %xmm0,%xmm1,%xmm0
     ea5:	c5 fa 52 c8          	vrsqrtss %xmm0,%xmm0,%xmm1
     ea9:	c5 fa 59 c1          	vmulss %xmm1,%xmm0,%xmm0
     ead:	c5 fa 59 c1          	vmulss %xmm1,%xmm0,%xmm0
     eb1:	c5 7a 10 25 00 00 00 	vmovss 0x0(%rip),%xmm12        # eb9 <main+0xe9>
     eb8:	00 
     eb9:	c5 9a 58 c0          	vaddss %xmm0,%xmm12,%xmm0
     ebd:	c5 fa 10 1d 00 00 00 	vmovss 0x0(%rip),%xmm3        # ec5 <main+0xf5>
     ec4:	00 
     ec5:	c5 f2 59 cb          	vmulss %xmm3,%xmm1,%xmm1
     ec9:	c5 78 28 d3          	vmovaps %xmm3,%xmm10
     ecd:	c5 f2 59 c0          	vmulss %xmm0,%xmm1,%xmm0
     ed1:	c5 fa 11 44 24 20    	vmovss %xmm0,0x20(%rsp)
     ed7:	c4 c1 12 5c cf       	vsubss %xmm15,%xmm13,%xmm1
     edc:	c5 f8 28 c5          	vmovaps %xmm5,%xmm0
     ee0:	c4 c1 7a 16 e9       	vmovshdup %xmm9,%xmm5
     ee5:	c5 ea 5c dd          	vsubss %xmm5,%xmm2,%xmm3
     ee9:	c4 c1 7a 5c f1       	vsubss %xmm9,%xmm0,%xmm6
     eee:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
     ef2:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     ef6:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
     efa:	c5 ca 58 c9          	vaddss %xmm1,%xmm6,%xmm1
     efe:	c5 e2 58 c9          	vaddss %xmm1,%xmm3,%xmm1
     f02:	c5 f2 52 d9          	vrsqrtss %xmm1,%xmm1,%xmm3
     f06:	c5 f2 59 cb          	vmulss %xmm3,%xmm1,%xmm1
     f0a:	c5 f2 59 cb          	vmulss %xmm3,%xmm1,%xmm1
     f0e:	c5 9a 58 c9          	vaddss %xmm1,%xmm12,%xmm1
     f12:	c5 aa 59 db          	vmulss %xmm3,%xmm10,%xmm3
     f16:	c5 e2 59 c9          	vmulss %xmm1,%xmm3,%xmm1
     f1a:	c5 fa 11 4c 24 10    	vmovss %xmm1,0x10(%rsp)
     f20:	c5 f8 c6 c8 ff       	vshufps $0xff,%xmm0,%xmm0,%xmm1
     f25:	c5 92 5c d9          	vsubss %xmm1,%xmm13,%xmm3
     f29:	c4 41 10 c6 d5 ff    	vshufps $0xff,%xmm13,%xmm13,%xmm10
     f2f:	c4 c1 6a 5c d2       	vsubss %xmm10,%xmm2,%xmm2
     f34:	c4 41 11 c6 dd 01    	vshufpd $0x1,%xmm13,%xmm13,%xmm11
     f3a:	c4 c1 7a 5c f3       	vsubss %xmm11,%xmm0,%xmm6
     f3f:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     f43:	c5 ea 59 d2          	vmulss %xmm2,%xmm2,%xmm2
     f47:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
     f4b:	c5 ea 58 d6          	vaddss %xmm6,%xmm2,%xmm2
     f4f:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
     f53:	c5 ea 52 da          	vrsqrtss %xmm2,%xmm2,%xmm3
     f57:	c5 ea 59 d3          	vmulss %xmm3,%xmm2,%xmm2
     f5b:	c5 ea 59 d3          	vmulss %xmm3,%xmm2,%xmm2
     f5f:	c5 9a 58 d2          	vaddss %xmm2,%xmm12,%xmm2
     f63:	c5 e2 59 1d 00 00 00 	vmulss 0x0(%rip),%xmm3,%xmm3        # f6b <main+0x19b>
     f6a:	00 
     f6b:	c5 e2 59 d2          	vmulss %xmm2,%xmm3,%xmm2
     f6f:	c5 fa 11 54 24 08    	vmovss %xmm2,0x8(%rsp)
     f75:	c4 c1 78 c6 de d6    	vshufps $0xd6,%xmm14,%xmm0,%xmm3
     f7b:	c4 c1 60 c6 de 48    	vshufps $0x48,%xmm14,%xmm3,%xmm3
     f81:	c5 78 29 ac 24 b0 00 	vmovaps %xmm13,0xb0(%rsp)
     f88:	00 00 
     f8a:	c5 90 5c f3          	vsubps %xmm3,%xmm13,%xmm6
     f8e:	c5 c8 59 d6          	vmulps %xmm6,%xmm6,%xmm2
     f92:	c5 f8 29 94 24 f0 00 	vmovaps %xmm2,0xf0(%rsp)
     f99:	00 00 
     f9b:	c4 c1 5a 5c df       	vsubss %xmm15,%xmm4,%xmm3
     fa0:	c5 42 5c e5          	vsubss %xmm5,%xmm7,%xmm12
     fa4:	c4 41 3a 5c e9       	vsubss %xmm9,%xmm8,%xmm13
     fa9:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     fad:	c4 41 1a 59 e4       	vmulss %xmm12,%xmm12,%xmm12
     fb2:	c4 41 12 59 ed       	vmulss %xmm13,%xmm13,%xmm13
     fb7:	c4 41 1a 58 e5       	vaddss %xmm13,%xmm12,%xmm12
     fbc:	c5 9a 58 db          	vaddss %xmm3,%xmm12,%xmm3
     fc0:	c5 62 52 e3          	vrsqrtss %xmm3,%xmm3,%xmm12
     fc4:	c5 9a 59 db          	vmulss %xmm3,%xmm12,%xmm3
     fc8:	c5 9a 59 db          	vmulss %xmm3,%xmm12,%xmm3
     fcc:	c5 7a 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm13        # fd4 <main+0x204>
     fd3:	00 
     fd4:	c5 92 58 db          	vaddss %xmm3,%xmm13,%xmm3
     fd8:	c5 fa 10 15 00 00 00 	vmovss 0x0(%rip),%xmm2        # fe0 <main+0x210>
     fdf:	00 
     fe0:	c5 1a 59 e2          	vmulss %xmm2,%xmm12,%xmm12
     fe4:	c5 9a 59 db          	vmulss %xmm3,%xmm12,%xmm3
     fe8:	c5 fa 11 5c 24 0c    	vmovss %xmm3,0xc(%rsp)
     fee:	c5 da 5c e1          	vsubss %xmm1,%xmm4,%xmm4
     ff2:	c4 c1 42 5c fa       	vsubss %xmm10,%xmm7,%xmm7
     ff7:	c4 41 3a 5c c3       	vsubss %xmm11,%xmm8,%xmm8
     ffc:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
    1000:	c5 c2 59 ff          	vmulss %xmm7,%xmm7,%xmm7
    1004:	c4 41 3a 59 c0       	vmulss %xmm8,%xmm8,%xmm8
    1009:	c5 ba 58 ff          	vaddss %xmm7,%xmm8,%xmm7
    100d:	c5 c2 58 e4          	vaddss %xmm4,%xmm7,%xmm4
    1011:	c5 da 52 fc          	vrsqrtss %xmm4,%xmm4,%xmm7
    1015:	c5 da 59 e7          	vmulss %xmm7,%xmm4,%xmm4
    1019:	c5 da 59 e7          	vmulss %xmm7,%xmm4,%xmm4
    101d:	c5 92 58 e4          	vaddss %xmm4,%xmm13,%xmm4
    1021:	c5 c2 59 fa          	vmulss %xmm2,%xmm7,%xmm7
    1025:	c5 c2 59 e4          	vmulss %xmm4,%xmm7,%xmm4
    1029:	c4 c1 00 c6 ff 39    	vshufps $0x39,%xmm15,%xmm15,%xmm7
    102f:	c4 e3 41 21 f8 30    	vinsertps $0x30,%xmm0,%xmm7,%xmm7
    1035:	c4 43 79 0c c6 03    	vblendps $0x3,%xmm14,%xmm0,%xmm8
    103b:	c4 41 38 c6 c0 24    	vshufps $0x24,%xmm8,%xmm8,%xmm8
    1041:	c4 41 40 5c e0       	vsubps %xmm8,%xmm7,%xmm12
    1046:	c5 82 5c f9          	vsubss %xmm1,%xmm15,%xmm7
    104a:	c4 c1 52 5c ea       	vsubss %xmm10,%xmm5,%xmm5
    104f:	c4 41 32 5c c3       	vsubss %xmm11,%xmm9,%xmm8
    1054:	c5 c2 59 ff          	vmulss %xmm7,%xmm7,%xmm7
    1058:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
    105c:	c4 41 3a 59 c0       	vmulss %xmm8,%xmm8,%xmm8
    1061:	c5 ba 58 ed          	vaddss %xmm5,%xmm8,%xmm5
    1065:	c5 d2 58 ef          	vaddss %xmm7,%xmm5,%xmm5
    1069:	c5 d2 52 fd          	vrsqrtss %xmm5,%xmm5,%xmm7
    106d:	c5 d2 59 ef          	vmulss %xmm7,%xmm5,%xmm5
    1071:	c5 d2 59 ef          	vmulss %xmm7,%xmm5,%xmm5
    1075:	c5 92 58 ed          	vaddss %xmm5,%xmm13,%xmm5
    1079:	c5 c2 59 fa          	vmulss %xmm2,%xmm7,%xmm7
    107d:	c5 c2 59 ed          	vmulss %xmm5,%xmm7,%xmm5
    1081:	c4 c1 78 c6 ff 27    	vshufps $0x27,%xmm15,%xmm0,%xmm7
    1087:	c5 78 29 8c 24 d0 00 	vmovaps %xmm9,0xd0(%rsp)
    108e:	00 00 
    1090:	c4 c1 40 c6 f9 4c    	vshufps $0x4c,%xmm9,%xmm7,%xmm7
    1096:	c5 c0 c6 ff 78       	vshufps $0x78,%xmm7,%xmm7,%xmm7
    109b:	c5 78 29 b4 24 c0 00 	vmovaps %xmm14,0xc0(%rsp)
    10a2:	00 00 
    10a4:	c4 41 78 c6 c6 4a    	vshufps $0x4a,%xmm14,%xmm0,%xmm8
    10aa:	c4 41 38 c6 c0 78    	vshufps $0x78,%xmm8,%xmm8,%xmm8
    10b0:	c4 c1 40 5c c8       	vsubps %xmm8,%xmm7,%xmm1
    10b5:	c4 c1 19 c6 fc 01    	vshufpd $0x1,%xmm12,%xmm12,%xmm7
    10bb:	c4 e3 41 21 f9 d0    	vinsertps $0xd0,%xmm1,%xmm7,%xmm7
    10c1:	c5 c0 c6 ff e1       	vshufps $0xe1,%xmm7,%xmm7,%xmm7
    10c6:	c5 c0 59 ff          	vmulps %xmm7,%xmm7,%xmm7
    10ca:	c5 18 c6 c1 43       	vshufps $0x43,%xmm1,%xmm12,%xmm8
    10cf:	c4 41 38 c6 c0 78    	vshufps $0x78,%xmm8,%xmm8,%xmm8
    10d5:	c4 41 38 59 c0       	vmulps %xmm8,%xmm8,%xmm8
    10da:	c5 f8 29 b4 24 a0 00 	vmovaps %xmm6,0xa0(%rsp)
    10e1:	00 00 
    10e3:	c5 fc 11 8c 24 30 01 	vmovups %ymm1,0x130(%rsp)
    10ea:	00 00 
    10ec:	c5 48 c6 c9 e5       	vshufps $0xe5,%xmm1,%xmm6,%xmm9
    10f1:	c5 f9 28 8c 24 f0 00 	vmovapd 0xf0(%rsp),%xmm1
    10f8:	00 00 
    10fa:	c5 71 c6 d1 01       	vshufpd $0x1,%xmm1,%xmm1,%xmm10
    10ff:	c5 70 c6 d9 aa       	vshufps $0xaa,%xmm1,%xmm1,%xmm11
    1104:	c4 41 28 58 d3       	vaddps %xmm11,%xmm10,%xmm10
    1109:	c5 78 29 a4 24 90 00 	vmovaps %xmm12,0x90(%rsp)
    1110:	00 00 
    1112:	c4 43 31 21 cc 72    	vinsertps $0x72,%xmm12,%xmm9,%xmm9
    1118:	c4 41 30 59 c9       	vmulps %xmm9,%xmm9,%xmm9
    111d:	c4 43 31 0c ca 02    	vblendps $0x2,%xmm10,%xmm9,%xmm9
    1123:	c4 41 30 58 c8       	vaddps %xmm8,%xmm9,%xmm9
    1128:	c4 41 7a 16 d1       	vmovshdup %xmm9,%xmm10
    112d:	c4 41 2a 52 c2       	vrsqrtss %xmm10,%xmm10,%xmm8
    1132:	c4 41 2a 59 d8       	vmulss %xmm8,%xmm10,%xmm11
    1137:	c4 41 22 59 d8       	vmulss %xmm8,%xmm11,%xmm11
    113c:	c4 41 22 58 dd       	vaddss %xmm13,%xmm11,%xmm11
    1141:	c5 f8 28 da          	vmovaps %xmm2,%xmm3
    1145:	c5 3a 59 c2          	vmulss %xmm2,%xmm8,%xmm8
    1149:	c4 41 3a 59 c3       	vmulss %xmm11,%xmm8,%xmm8
    114e:	c4 41 2a 51 d2       	vsqrtss %xmm10,%xmm10,%xmm10
    1153:	c4 c3 71 21 f2 10    	vinsertps $0x10,%xmm10,%xmm1,%xmm6
    1159:	c5 c8 16 ff          	vmovlhps %xmm7,%xmm6,%xmm7
    115d:	c5 30 58 d7          	vaddps %xmm7,%xmm9,%xmm10
    1161:	c5 b0 59 f6          	vmulps %xmm6,%xmm9,%xmm6
    1165:	c4 e3 29 0c ce 02    	vblendps $0x2,%xmm6,%xmm10,%xmm1
    116b:	c5 f8 53 f9          	vrcpps %xmm1,%xmm7
    116f:	c4 c1 29 c6 f2 01    	vshufpd $0x1,%xmm10,%xmm10,%xmm6
    1175:	c5 f8 51 f6          	vsqrtps %xmm6,%xmm6
    1179:	c4 62 7d 18 0d 00 00 	vbroadcastss 0x0(%rip),%ymm9        # 1182 <main+0x3b2>
    1180:	00 00 
    1182:	c4 41 32 51 ca       	vsqrtss %xmm10,%xmm9,%xmm9
    1187:	c5 b0 16 d6          	vmovlhps %xmm6,%xmm9,%xmm2
    118b:	c5 68 59 cf          	vmulps %xmm7,%xmm2,%xmm9
    118f:	c5 f8 29 4c 24 60    	vmovaps %xmm1,0x60(%rsp)
    1195:	c5 30 59 d9          	vmulps %xmm1,%xmm9,%xmm11
    1199:	c5 f8 29 54 24 50    	vmovaps %xmm2,0x50(%rsp)
    119f:	c4 41 68 5c db       	vsubps %xmm11,%xmm2,%xmm11
    11a4:	c5 a0 59 ff          	vmulps %xmm7,%xmm11,%xmm7
    11a8:	c5 b0 58 ff          	vaddps %xmm7,%xmm9,%xmm7
    11ac:	c4 41 2a 52 ca       	vrsqrtss %xmm10,%xmm10,%xmm9
    11b1:	c4 41 2a 59 d1       	vmulss %xmm9,%xmm10,%xmm10
    11b6:	c4 41 2a 59 d1       	vmulss %xmm9,%xmm10,%xmm10
    11bb:	c4 41 2a 58 d5       	vaddss %xmm13,%xmm10,%xmm10
    11c0:	c5 32 59 cb          	vmulss %xmm3,%xmm9,%xmm9
    11c4:	c4 41 32 59 ca       	vmulss %xmm10,%xmm9,%xmm9
    11c9:	c5 7c 11 bc 24 f0 00 	vmovups %ymm15,0xf0(%rsp)
    11d0:	00 00 
    11d2:	c4 63 7d 19 f9 01    	vextractf128 $0x1,%ymm15,%xmm1
    11d8:	c5 7a 16 d1          	vmovshdup %xmm1,%xmm10
    11dc:	c4 41 2a 59 d2       	vmulss %xmm10,%xmm10,%xmm10
    11e1:	c5 72 59 d9          	vmulss %xmm1,%xmm1,%xmm11
    11e5:	c4 41 2a 58 d3       	vaddss %xmm11,%xmm10,%xmm10
    11ea:	c5 7d 10 bc 24 10 01 	vmovupd 0x110(%rsp),%ymm15
    11f1:	00 00 
    11f3:	c4 43 7d 19 fd 01    	vextractf128 $0x1,%ymm15,%xmm13
    11f9:	c4 c1 10 c6 d5 ff    	vshufps $0xff,%xmm13,%xmm13,%xmm2
    11ff:	c5 f8 29 54 24 70    	vmovaps %xmm2,0x70(%rsp)
    1205:	c5 6a 59 da          	vmulss %xmm2,%xmm2,%xmm11
    1209:	c4 41 2a 58 d3       	vaddss %xmm11,%xmm10,%xmm10
    120e:	c4 41 7a 16 dd       	vmovshdup %xmm13,%xmm11
    1213:	c4 41 22 59 db       	vmulss %xmm11,%xmm11,%xmm11
    1218:	c4 41 12 59 e5       	vmulss %xmm13,%xmm13,%xmm12
    121d:	c4 41 22 58 dc       	vaddss %xmm12,%xmm11,%xmm11
    1222:	c5 78 29 6c 24 40    	vmovaps %xmm13,0x40(%rsp)
    1228:	c4 41 11 c6 e5 01    	vshufpd $0x1,%xmm13,%xmm13,%xmm12
    122e:	c4 41 1a 59 e4       	vmulss %xmm12,%xmm12,%xmm12
    1233:	c4 41 22 58 e4       	vaddss %xmm12,%xmm11,%xmm12
    1238:	c4 41 01 c6 df 01    	vshufpd $0x1,%xmm15,%xmm15,%xmm11
    123e:	c4 41 22 59 db       	vmulss %xmm11,%xmm11,%xmm11
    1243:	c4 41 7a 16 ef       	vmovshdup %xmm15,%xmm13
    1248:	c4 41 12 59 ed       	vmulss %xmm13,%xmm13,%xmm13
    124d:	c4 41 22 58 dd       	vaddss %xmm13,%xmm11,%xmm11
    1252:	c4 41 00 c6 ef ff    	vshufps $0xff,%xmm15,%xmm15,%xmm13
    1258:	c4 41 12 59 ed       	vmulss %xmm13,%xmm13,%xmm13
    125d:	c4 41 22 58 dd       	vaddss %xmm13,%xmm11,%xmm11
    1262:	c5 70 c6 e9 ff       	vshufps $0xff,%xmm1,%xmm1,%xmm13
    1267:	c4 41 12 59 ed       	vmulss %xmm13,%xmm13,%xmm13
    126c:	c5 f8 29 8c 24 80 00 	vmovaps %xmm1,0x80(%rsp)
    1273:	00 00 
    1275:	c5 71 c6 f1 01       	vshufpd $0x1,%xmm1,%xmm1,%xmm14
    127a:	c4 41 0a 59 f6       	vmulss %xmm14,%xmm14,%xmm14
    127f:	c4 41 12 58 ee       	vaddss %xmm14,%xmm13,%xmm13
    1284:	c4 41 02 59 f7       	vmulss %xmm15,%xmm15,%xmm14
    1289:	c4 41 12 58 ee       	vaddss %xmm14,%xmm13,%xmm13
    128e:	c5 78 28 74 24 30    	vmovaps 0x30(%rsp),%xmm14
    1294:	c4 41 08 59 f6       	vmulps %xmm14,%xmm14,%xmm14
    1299:	c4 41 7a 16 fe       	vmovshdup %xmm14,%xmm15
    129e:	c4 41 02 58 f6       	vaddss %xmm14,%xmm15,%xmm14
    12a3:	c5 f8 29 84 24 e0 00 	vmovaps %xmm0,0xe0(%rsp)
    12aa:	00 00 
    12ac:	c5 78 59 f8          	vmulps %xmm0,%xmm0,%xmm15
    12b0:	c4 41 7a 16 ff       	vmovshdup %xmm15,%xmm15
    12b5:	c4 41 0a 58 f7       	vaddss %xmm15,%xmm14,%xmm14
    12ba:	c5 12 59 2d 00 00 00 	vmulss 0x0(%rip),%xmm13,%xmm13        # 12c2 <main+0x4f2>
    12c1:	00 
    12c2:	c5 0a 59 35 00 00 00 	vmulss 0x0(%rip),%xmm14,%xmm14        # 12ca <main+0x4fa>
    12c9:	00 
    12ca:	c4 41 0a 58 ed       	vaddss %xmm13,%xmm14,%xmm13
    12cf:	c5 1a 59 25 00 00 00 	vmulss 0x0(%rip),%xmm12,%xmm12        # 12d7 <main+0x507>
    12d6:	00 
    12d7:	c5 fa 10 05 00 00 00 	vmovss 0x0(%rip),%xmm0        # 12df <main+0x50f>
    12de:	00 
    12df:	c5 7a 5e f6          	vdivss %xmm6,%xmm0,%xmm14
    12e3:	c4 41 0a 58 e4       	vaddss %xmm12,%xmm14,%xmm12
    12e8:	c5 fa 10 44 24 08    	vmovss 0x8(%rsp),%xmm0
    12ee:	c5 fa 59 15 00 00 00 	vmulss 0x0(%rip),%xmm0,%xmm2        # 12f6 <main+0x526>
    12f5:	00 
    12f6:	c5 92 58 d2          	vaddss %xmm2,%xmm13,%xmm2
    12fa:	c5 3a 59 05 00 00 00 	vmulss 0x0(%rip),%xmm8,%xmm8        # 1302 <main+0x532>
    1301:	00 
    1302:	c5 ba 58 d2          	vaddss %xmm2,%xmm8,%xmm2
    1306:	c5 22 59 05 00 00 00 	vmulss 0x0(%rip),%xmm11,%xmm8        # 130e <main+0x53e>
    130d:	00 
    130e:	c5 ba 58 d2          	vaddss %xmm2,%xmm8,%xmm2
    1312:	c5 fa 10 44 24 10    	vmovss 0x10(%rsp),%xmm0
    1318:	c5 fa 59 0d 00 00 00 	vmulss 0x0(%rip),%xmm0,%xmm1        # 1320 <main+0x550>
    131f:	00 
    1320:	c5 ea 58 c9          	vaddss %xmm1,%xmm2,%xmm1
    1324:	c5 d2 59 15 00 00 00 	vmulss 0x0(%rip),%xmm5,%xmm2        # 132c <main+0x55c>
    132b:	00 
    132c:	c5 f2 58 ca          	vaddss %xmm2,%xmm1,%xmm1
    1330:	c5 b2 59 15 00 00 00 	vmulss 0x0(%rip),%xmm9,%xmm2        # 1338 <main+0x568>
    1337:	00 
    1338:	c5 ea 58 c9          	vaddss %xmm1,%xmm2,%xmm1
    133c:	c5 9a 58 c9          	vaddss %xmm1,%xmm12,%xmm1
    1340:	c5 aa 59 15 00 00 00 	vmulss 0x0(%rip),%xmm10,%xmm2        # 1348 <main+0x578>
    1347:	00 
    1348:	c5 fa 10 44 24 20    	vmovss 0x20(%rsp),%xmm0
    134e:	c5 fa 59 05 00 00 00 	vmulss 0x0(%rip),%xmm0,%xmm0        # 1356 <main+0x586>
    1355:	00 
    1356:	c5 ea 58 c0          	vaddss %xmm0,%xmm2,%xmm0
    135a:	c5 fa 10 54 24 0c    	vmovss 0xc(%rsp),%xmm2
    1360:	c5 ea 59 15 00 00 00 	vmulss 0x0(%rip),%xmm2,%xmm2        # 1368 <main+0x598>
    1367:	00 
    1368:	c5 fa 58 c2          	vaddss %xmm2,%xmm0,%xmm0
    136c:	c5 da 59 15 00 00 00 	vmulss 0x0(%rip),%xmm4,%xmm2        # 1374 <main+0x5a4>
    1373:	00 
    1374:	c5 fa 58 c2          	vaddss %xmm2,%xmm0,%xmm0
    1378:	c5 fa 16 d6          	vmovshdup %xmm6,%xmm2
    137c:	c5 fa 10 1d 00 00 00 	vmovss 0x0(%rip),%xmm3        # 1384 <main+0x5b4>
    1383:	00 
    1384:	c5 e2 5e d2          	vdivss %xmm2,%xmm3,%xmm2
    1388:	c5 fa 58 c2          	vaddss %xmm2,%xmm0,%xmm0
    138c:	c5 f2 58 c0          	vaddss %xmm0,%xmm1,%xmm0
    1390:	c5 c0 59 2d 00 00 00 	vmulps 0x0(%rip),%xmm7,%xmm5        # 1398 <main+0x5c8>
    1397:	00 
    1398:	c5 fa 16 cf          	vmovshdup %xmm7,%xmm1
    139c:	c5 72 59 05 00 00 00 	vmulss 0x0(%rip),%xmm1,%xmm8        # 13a4 <main+0x5d4>
    13a3:	00 
    13a4:	4c 89 f0             	mov    %r14,%rax
    13a7:	49 0f af c7          	imul   %r15,%rax
    13ab:	4c 01 e0             	add    %r12,%rax
    13ae:	48 0f ac c0 06       	shrd   $0x6,%rax,%rax
    13b3:	4c 39 e8             	cmp    %r13,%rax
    13b6:	77 2c                	ja     13e4 <main+0x614>
    13b8:	c5 78 29 44 24 20    	vmovaps %xmm8,0x20(%rsp)
    13be:	c5 f8 29 6c 24 10    	vmovaps %xmm5,0x10(%rsp)
    13c4:	c5 fa 11 44 24 08    	vmovss %xmm0,0x8(%rsp)
    13ca:	c5 f8 77             	vzeroupper
    13cd:	e8 00 00 00 00       	call   13d2 <main+0x602>
    13d2:	c5 fa 10 44 24 08    	vmovss 0x8(%rsp),%xmm0
    13d8:	c5 f8 28 6c 24 10    	vmovaps 0x10(%rsp),%xmm5
    13de:	c5 78 28 44 24 20    	vmovaps 0x20(%rsp),%xmm8
    13e4:	49 39 de             	cmp    %rbx,%r14
    13e7:	0f 84 51 01 00 00    	je     153e <main+0x76e>
    13ed:	c5 f8 28 44 24 50    	vmovaps 0x50(%rsp),%xmm0
    13f3:	c5 f8 59 44 24 60    	vmulps 0x60(%rsp),%xmm0,%xmm0
    13f9:	c5 f8 53 c8          	vrcpps %xmm0,%xmm1
    13fd:	c4 e2 79 18 1d 00 00 	vbroadcastss 0x0(%rip),%xmm3        # 1406 <main+0x636>
    1404:	00 00 
    1406:	c5 f0 59 d3          	vmulps %xmm3,%xmm1,%xmm2
    140a:	c5 f8 59 c2          	vmulps %xmm2,%xmm0,%xmm0
    140e:	c5 e0 5c c0          	vsubps %xmm0,%xmm3,%xmm0
    1412:	c5 f0 59 c0          	vmulps %xmm0,%xmm1,%xmm0
    1416:	c5 e8 58 c8          	vaddps %xmm0,%xmm2,%xmm1
    141a:	c4 c1 7a 12 c0       	vmovsldup %xmm8,%xmm0
    141f:	c5 f9 28 a4 24 a0 00 	vmovapd 0xa0(%rsp),%xmm4
    1426:	00 00 
    1428:	c5 d9 c6 d4 01       	vshufpd $0x1,%xmm4,%xmm4,%xmm2
    142d:	c5 f8 59 c2          	vmulps %xmm2,%xmm0,%xmm0
    1431:	c4 e3 71 0c d5 02    	vblendps $0x2,%xmm5,%xmm1,%xmm2
    1437:	c5 fc 10 ac 24 10 01 	vmovups 0x110(%rsp),%ymm5
    143e:	00 00 
    1440:	c4 e3 51 0c 5c 24 40 	vblendps $0x7,0x40(%rsp),%xmm5,%xmm3
    1447:	07 
    1448:	c5 e0 c6 db 93       	vshufps $0x93,%xmm3,%xmm3,%xmm3
    144d:	c4 e3 65 18 da 01    	vinsertf128 $0x1,%xmm2,%ymm3,%ymm3
    1453:	c5 e8 c6 d2 a9       	vshufps $0xa9,%xmm2,%xmm2,%xmm2
    1458:	c5 f0 c6 c9 3f       	vshufps $0x3f,%xmm1,%xmm1,%xmm1
    145d:	c4 e3 6d 18 c9 01    	vinsertf128 $0x1,%xmm1,%ymm2,%ymm1
    1463:	c4 e2 65 0c 15 00 00 	vpermilps 0x0(%rip),%ymm3,%ymm2        # 146c <main+0x69c>
    146a:	00 00 
    146c:	c4 e2 7d 18 35 00 00 	vbroadcastss 0x0(%rip),%ymm6        # 1475 <main+0x6a5>
    1473:	00 00 
    1475:	c4 e3 4d 18 dc 01    	vinsertf128 $0x1,%xmm4,%ymm6,%ymm3
    147b:	c5 ec 59 d3          	vmulps %ymm3,%ymm2,%ymm2
    147f:	c5 fc 10 bc 24 30 01 	vmovups 0x130(%rsp),%ymm7
    1486:	00 00 
    1488:	c4 e3 45 18 9c 24 90 	vinsertf128 $0x1,0x90(%rsp),%ymm7,%ymm3
    148f:	00 00 00 01 
    1493:	c5 f4 59 cb          	vmulps %ymm3,%ymm1,%ymm1
    1497:	c5 fc 10 a4 24 f0 00 	vmovups 0xf0(%rsp),%ymm4
    149e:	00 00 
    14a0:	c5 dc 58 da          	vaddps %ymm2,%ymm4,%ymm3
    14a4:	c5 dc 5c d2          	vsubps %ymm2,%ymm4,%ymm2
    14a8:	c4 63 65 0c fa f0    	vblendps $0xf0,%ymm2,%ymm3,%ymm15
    14ae:	c5 f8 28 54 24 70    	vmovaps 0x70(%rsp),%xmm2
    14b4:	c4 c3 69 21 d0 1c    	vinsertps $0x1c,%xmm8,%xmm2,%xmm2
    14ba:	c5 e8 c6 d5 24       	vshufps $0x24,%xmm5,%xmm2,%xmm2
    14bf:	c5 d4 5c c9          	vsubps %ymm1,%ymm5,%ymm1
    14c3:	c5 d0 c6 dd e9       	vshufps $0xe9,%xmm5,%xmm5,%xmm3
    14c8:	c5 e0 59 de          	vmulps %xmm6,%xmm3,%xmm3
    14cc:	c5 78 28 8c 24 d0 00 	vmovaps 0xd0(%rsp),%xmm9
    14d3:	00 00 
    14d5:	c5 30 58 cb          	vaddps %xmm3,%xmm9,%xmm9
    14d9:	c5 c8 59 9c 24 80 00 	vmulps 0x80(%rsp),%xmm6,%xmm3
    14e0:	00 00 
    14e2:	c5 78 28 ac 24 b0 00 	vmovaps 0xb0(%rsp),%xmm13
    14e9:	00 00 
    14eb:	c5 10 58 eb          	vaddps %xmm3,%xmm13,%xmm13
    14ef:	c5 f8 28 ac 24 e0 00 	vmovaps 0xe0(%rsp),%xmm5
    14f6:	00 00 
    14f8:	c4 e3 69 21 d5 60    	vinsertps $0x60,%xmm5,%xmm2,%xmm2
    14fe:	c4 e3 49 21 df 10    	vinsertps $0x10,%xmm7,%xmm6,%xmm3
    1504:	c5 e8 59 d3          	vmulps %xmm3,%xmm2,%xmm2
    1508:	c5 e8 58 ed          	vaddps %xmm5,%xmm2,%xmm5
    150c:	c5 f8 28 54 24 30    	vmovaps 0x30(%rsp),%xmm2
    1512:	c5 f8 58 c2          	vaddps %xmm2,%xmm0,%xmm0
    1516:	c5 e8 59 d6          	vmulps %xmm6,%xmm2,%xmm2
    151a:	c5 78 28 b4 24 c0 00 	vmovaps 0xc0(%rsp),%xmm14
    1521:	00 00 
    1523:	c5 08 58 f2          	vaddps %xmm2,%xmm14,%xmm14
    1527:	49 ff c6             	inc    %r14
    152a:	c5 fc 11 8c 24 10 01 	vmovups %ymm1,0x110(%rsp)
    1531:	00 00 
    1533:	c5 f8 29 44 24 30    	vmovaps %xmm0,0x30(%rsp)
    1539:	e9 22 f9 ff ff       	jmp    e60 <main+0x90>
    153e:	c5 f8 77             	vzeroupper
    1541:	e8 00 00 00 00       	call   1546 <main+0x776>
    1546:	31 c0                	xor    %eax,%eax
    1548:	48 81 c4 50 01 00 00 	add    $0x150,%rsp
    154f:	5b                   	pop    %rbx
    1550:	41 5c                	pop    %r12
    1552:	41 5d                	pop    %r13
    1554:	41 5e                	pop    %r14
    1556:	41 5f                	pop    %r15
    1558:	c5 f8 77             	vzeroupper
    155b:	c3                   	ret
    155c:	0f 1f 40 00          	nopl   0x0(%rax)

0000000000001560 <brief_rt_ctor>:
    1560:	e9 00 00 00 00       	jmp    1565 <brief_rt_ctor+0x5>
    1565:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
    156c:	00 00 00 00 

0000000000001570 <__rt_init>:
    1570:	53                   	push   %rbx
    1571:	48 81 ec 00 01 00 00 	sub    $0x100,%rsp
    1578:	be 00 00 00 00       	mov    $0x0,%esi
    157d:	bf 02 00 00 00       	mov    $0x2,%edi
    1582:	e8 00 00 00 00       	call   1587 <__rt_init+0x17>
    1587:	be 00 00 00 00       	mov    $0x0,%esi
    158c:	bf 0f 00 00 00       	mov    $0xf,%edi
    1591:	e8 00 00 00 00       	call   1596 <__rt_init+0x26>
    1596:	be 00 00 00 00       	mov    $0x0,%esi
    159b:	bf 01 00 00 00       	mov    $0x1,%edi
    15a0:	e8 00 00 00 00       	call   15a5 <__rt_init+0x35>
    15a5:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    15a9:	c5 fc 11 84 24 e0 00 	vmovups %ymm0,0xe0(%rsp)
    15b0:	00 00 
    15b2:	c5 fc 11 84 24 d0 00 	vmovups %ymm0,0xd0(%rsp)
    15b9:	00 00 
    15bb:	c5 fc 11 84 24 b0 00 	vmovups %ymm0,0xb0(%rsp)
    15c2:	00 00 
    15c4:	c5 fc 11 84 24 90 00 	vmovups %ymm0,0x90(%rsp)
    15cb:	00 00 
    15cd:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
    15d3:	48 c7 44 24 68 00 00 	movq   $0x0,0x68(%rsp)
    15da:	00 00 
    15dc:	c7 84 24 f0 00 00 00 	movl   $0x4,0xf0(%rsp)
    15e3:	04 00 00 00 
    15e7:	c5 f8 77             	vzeroupper
    15ea:	e8 00 00 00 00       	call   15ef <__rt_init+0x7f>
    15ef:	8d 78 01             	lea    0x1(%rax),%edi
    15f2:	48 8d 5c 24 68       	lea    0x68(%rsp),%rbx
    15f7:	48 89 de             	mov    %rbx,%rsi
    15fa:	31 d2                	xor    %edx,%edx
    15fc:	e8 00 00 00 00       	call   1601 <__rt_init+0x91>
    1601:	e8 00 00 00 00       	call   1606 <__rt_init+0x96>
    1606:	8d 78 02             	lea    0x2(%rax),%edi
    1609:	48 89 de             	mov    %rbx,%rsi
    160c:	31 d2                	xor    %edx,%edx
    160e:	e8 00 00 00 00       	call   1613 <__rt_init+0xa3>
    1613:	e8 00 00 00 00       	call   1618 <__rt_init+0xa8>
    1618:	ff c0                	inc    %eax
    161a:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    161e:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
    1623:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
    1629:	89 44 24 08          	mov    %eax,0x8(%rsp)
    162d:	48 89 e6             	mov    %rsp,%rsi
    1630:	ba 00 00 00 00       	mov    $0x0,%edx
    1635:	31 ff                	xor    %edi,%edi
    1637:	c5 f8 77             	vzeroupper
    163a:	e8 00 00 00 00       	call   163f <__rt_init+0xcf>
    163f:	85 c0                	test   %eax,%eax
    1641:	75 27                	jne    166a <__rt_init+0xfa>
    1643:	c4 e2 7d 1a 05 00 00 	vbroadcastf128 0x0(%rip),%ymm0        # 164c <__rt_init+0xdc>
    164a:	00 00 
    164c:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
    1652:	48 8b 3d 00 00 00 00 	mov    0x0(%rip),%rdi        # 1659 <__rt_init+0xe9>
    1659:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
    165e:	31 f6                	xor    %esi,%esi
    1660:	31 c9                	xor    %ecx,%ecx
    1662:	c5 f8 77             	vzeroupper
    1665:	e8 00 00 00 00       	call   166a <__rt_init+0xfa>
    166a:	e8 00 00 00 00       	call   166f <__rt_init+0xff>
    166f:	83 c0 02             	add    $0x2,%eax
    1672:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    1676:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
    167b:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
    1681:	89 44 24 08          	mov    %eax,0x8(%rsp)
    1685:	48 89 e6             	mov    %rsp,%rsi
    1688:	ba 00 00 00 00       	mov    $0x0,%edx
    168d:	31 ff                	xor    %edi,%edi
    168f:	c5 f8 77             	vzeroupper
    1692:	e8 00 00 00 00       	call   1697 <__rt_init+0x127>
    1697:	85 c0                	test   %eax,%eax
    1699:	75 27                	jne    16c2 <__rt_init+0x152>
    169b:	c4 e2 7d 1a 05 00 00 	vbroadcastf128 0x0(%rip),%ymm0        # 16a4 <__rt_init+0x134>
    16a2:	00 00 
    16a4:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
    16aa:	48 8b 3d 00 00 00 00 	mov    0x0(%rip),%rdi        # 16b1 <__rt_init+0x141>
    16b1:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
    16b6:	31 f6                	xor    %esi,%esi
    16b8:	31 c9                	xor    %ecx,%ecx
    16ba:	c5 f8 77             	vzeroupper
    16bd:	e8 00 00 00 00       	call   16c2 <__rt_init+0x152>
    16c2:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 16c9 <__rt_init+0x159>
    16c9:	48 8b 38             	mov    (%rax),%rdi
    16cc:	31 f6                	xor    %esi,%esi
    16ce:	ba 01 00 00 00       	mov    $0x1,%edx
    16d3:	31 c9                	xor    %ecx,%ecx
    16d5:	e8 00 00 00 00       	call   16da <__rt_init+0x16a>
    16da:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 16e1 <__rt_init+0x171>
    16e1:	48 81 c4 00 01 00 00 	add    $0x100,%rsp
    16e8:	5b                   	pop    %rbx
    16e9:	c3                   	ret
    16ea:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

00000000000016f0 <handle_sigint>:
    16f0:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 16f7 <handle_sigint+0x7>
    16f7:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 16fe <handle_sigint+0xe>
    16fe:	c3                   	ret
    16ff:	90                   	nop

0000000000001700 <handle_sigterm>:
    1700:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1707 <handle_sigterm+0x7>
    1707:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 170e <handle_sigterm+0xe>
    170e:	c3                   	ret
    170f:	90                   	nop

0000000000001710 <handle_sighup>:
    1710:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1717 <handle_sighup+0x7>
    1717:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 171e <handle_sighup+0xe>
    171e:	c3                   	ret
    171f:	90                   	nop

0000000000001720 <handle_timer>:
    1720:	48 ff 05 00 00 00 00 	incq   0x0(%rip)        # 1727 <handle_timer+0x7>
    1727:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 172e <handle_timer+0xe>
    172e:	c3                   	ret
    172f:	90                   	nop

0000000000001730 <__get_env_int>:
    1730:	53                   	push   %rbx
    1731:	48 83 ec 10          	sub    $0x10,%rsp
    1735:	e8 00 00 00 00       	call   173a <__get_env_int+0xa>
    173a:	48 85 c0             	test   %rax,%rax
    173d:	74 32                	je     1771 <__get_env_int+0x41>
    173f:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
    1746:	00 00 
    1748:	48 8d 74 24 08       	lea    0x8(%rsp),%rsi
    174d:	48 89 c7             	mov    %rax,%rdi
    1750:	ba 0a 00 00 00       	mov    $0xa,%edx
    1755:	48 89 c3             	mov    %rax,%rbx
    1758:	e8 00 00 00 00       	call   175d <__get_env_int+0x2d>
    175d:	48 89 c1             	mov    %rax,%rcx
    1760:	31 c0                	xor    %eax,%eax
    1762:	48 39 5c 24 08       	cmp    %rbx,0x8(%rsp)
    1767:	48 0f 45 c1          	cmovne %rcx,%rax
    176b:	48 83 c4 10          	add    $0x10,%rsp
    176f:	5b                   	pop    %rbx
    1770:	c3                   	ret
    1771:	31 c0                	xor    %eax,%eax
    1773:	48 83 c4 10          	add    $0x10,%rsp
    1777:	5b                   	pop    %rbx
    1778:	c3                   	ret
    1779:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

0000000000001780 <__rt_wait>:
    1780:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
    1787:	8b 3d 00 00 00 00    	mov    0x0(%rip),%edi        # 178d <__rt_wait+0xd>
    178d:	85 ff                	test   %edi,%edi
    178f:	79 3f                	jns    17d0 <__rt_wait+0x50>
    1791:	31 ff                	xor    %edi,%edi
    1793:	e8 00 00 00 00       	call   1798 <__rt_wait+0x18>
    1798:	89 05 00 00 00 00    	mov    %eax,0x0(%rip)        # 179e <__rt_wait+0x1e>
    179e:	85 c0                	test   %eax,%eax
    17a0:	0f 88 d5 00 00 00    	js     187b <__rt_wait+0xfb>
    17a6:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
    17ad:	00 
    17ae:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
    17b5:	00 00 
    17b7:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
    17bc:	89 c7                	mov    %eax,%edi
    17be:	be 01 00 00 00       	mov    $0x1,%esi
    17c3:	31 d2                	xor    %edx,%edx
    17c5:	e8 00 00 00 00       	call   17ca <__rt_wait+0x4a>
    17ca:	8b 3d 00 00 00 00    	mov    0x0(%rip),%edi        # 17d0 <__rt_wait+0x50>
    17d0:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
    17d5:	ba 40 00 00 00       	mov    $0x40,%edx
    17da:	b9 64 00 00 00       	mov    $0x64,%ecx
    17df:	e8 00 00 00 00       	call   17e4 <__rt_wait+0x64>
    17e4:	85 c0                	test   %eax,%eax
    17e6:	0f 8e ef 00 00 00    	jle    18db <__rt_wait+0x15b>
    17ec:	89 c1                	mov    %eax,%ecx
    17ee:	83 f8 01             	cmp    $0x1,%eax
    17f1:	75 1e                	jne    1811 <__rt_wait+0x91>
    17f3:	31 c0                	xor    %eax,%eax
    17f5:	f6 c1 01             	test   $0x1,%cl
    17f8:	74 0f                	je     1809 <__rt_wait+0x89>
    17fa:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
    17fe:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
    1803:	0f 84 e1 00 00 00    	je     18ea <__rt_wait+0x16a>
    1809:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
    1810:	c3                   	ret
    1811:	89 c8                	mov    %ecx,%eax
    1813:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
    1818:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
    181d:	48 89 c6             	mov    %rax,%rsi
    1820:	eb 18                	jmp    183a <__rt_wait+0xba>
    1822:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1829:	1f 84 00 00 00 00 00 
    1830:	48 83 c2 18          	add    $0x18,%rdx
    1834:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
    1838:	74 bb                	je     17f5 <__rt_wait+0x75>
    183a:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
    183e:	75 20                	jne    1860 <__rt_wait+0xe0>
    1840:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
    1844:	74 1a                	je     1860 <__rt_wait+0xe0>
    1846:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 184d <__rt_wait+0xcd>
    184d:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1854 <__rt_wait+0xd4>
    1854:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
    185b:	00 00 00 00 00 
    1860:	83 3a 00             	cmpl   $0x0,(%rdx)
    1863:	75 cb                	jne    1830 <__rt_wait+0xb0>
    1865:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
    1869:	74 c5                	je     1830 <__rt_wait+0xb0>
    186b:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1872 <__rt_wait+0xf2>
    1872:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1879 <__rt_wait+0xf9>
    1879:	eb b5                	jmp    1830 <__rt_wait+0xb0>
    187b:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    187f:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
    1885:	c5 fc 11 44 24 58    	vmovups %ymm0,0x58(%rsp)
    188b:	c5 fc 11 44 24 38    	vmovups %ymm0,0x38(%rsp)
    1891:	c5 fc 11 44 24 18    	vmovups %ymm0,0x18(%rsp)
    1897:	48 c7 44 24 10 01 00 	movq   $0x1,0x10(%rsp)
    189e:	00 00 
    18a0:	c5 f8 10 05 00 00 00 	vmovups 0x0(%rip),%xmm0        # 18a8 <__rt_wait+0x128>
    18a7:	00 
    18a8:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
    18ad:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
    18b2:	49 89 e0             	mov    %rsp,%r8
    18b5:	bf 01 00 00 00       	mov    $0x1,%edi
    18ba:	31 d2                	xor    %edx,%edx
    18bc:	31 c9                	xor    %ecx,%ecx
    18be:	c5 f8 77             	vzeroupper
    18c1:	e8 00 00 00 00       	call   18c6 <__rt_wait+0x146>
    18c6:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
    18cb:	74 0e                	je     18db <__rt_wait+0x15b>
    18cd:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 18d4 <__rt_wait+0x154>
    18d4:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 18db <__rt_wait+0x15b>
    18db:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 18e2 <__rt_wait+0x162>
    18e2:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
    18e9:	c3                   	ret
    18ea:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
    18ef:	0f 84 14 ff ff ff    	je     1809 <__rt_wait+0x89>
    18f5:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 18fc <__rt_wait+0x17c>
    18fc:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1903 <__rt_wait+0x183>
    1903:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
    190a:	c3                   	ret
    190b:	0f 1f 44 00 00       	nopl   0x0(%rax,%rax,1)

0000000000001910 <__rt_poll>:
    1910:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
    1917:	8b 3d 00 00 00 00    	mov    0x0(%rip),%edi        # 191d <__rt_poll+0xd>
    191d:	85 ff                	test   %edi,%edi
    191f:	79 3f                	jns    1960 <__rt_poll+0x50>
    1921:	31 ff                	xor    %edi,%edi
    1923:	e8 00 00 00 00       	call   1928 <__rt_poll+0x18>
    1928:	89 05 00 00 00 00    	mov    %eax,0x0(%rip)        # 192e <__rt_poll+0x1e>
    192e:	85 c0                	test   %eax,%eax
    1930:	0f 88 d5 00 00 00    	js     1a0b <__rt_poll+0xfb>
    1936:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
    193d:	00 
    193e:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
    1945:	00 00 
    1947:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
    194c:	89 c7                	mov    %eax,%edi
    194e:	be 01 00 00 00       	mov    $0x1,%esi
    1953:	31 d2                	xor    %edx,%edx
    1955:	e8 00 00 00 00       	call   195a <__rt_poll+0x4a>
    195a:	8b 3d 00 00 00 00    	mov    0x0(%rip),%edi        # 1960 <__rt_poll+0x50>
    1960:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
    1965:	ba 40 00 00 00       	mov    $0x40,%edx
    196a:	31 c9                	xor    %ecx,%ecx
    196c:	e8 00 00 00 00       	call   1971 <__rt_poll+0x61>
    1971:	85 c0                	test   %eax,%eax
    1973:	7e 1d                	jle    1992 <__rt_poll+0x82>
    1975:	89 c1                	mov    %eax,%ecx
    1977:	83 f8 01             	cmp    $0x1,%eax
    197a:	75 25                	jne    19a1 <__rt_poll+0x91>
    197c:	31 c0                	xor    %eax,%eax
    197e:	f6 c1 01             	test   $0x1,%cl
    1981:	74 0f                	je     1992 <__rt_poll+0x82>
    1983:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
    1987:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
    198c:	0f 84 cd 00 00 00    	je     1a5f <__rt_poll+0x14f>
    1992:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1999 <__rt_poll+0x89>
    1999:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
    19a0:	c3                   	ret
    19a1:	89 c8                	mov    %ecx,%eax
    19a3:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
    19a8:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
    19ad:	48 89 c6             	mov    %rax,%rsi
    19b0:	eb 18                	jmp    19ca <__rt_poll+0xba>
    19b2:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    19b9:	1f 84 00 00 00 00 00 
    19c0:	48 83 c2 18          	add    $0x18,%rdx
    19c4:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
    19c8:	74 b4                	je     197e <__rt_poll+0x6e>
    19ca:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
    19ce:	75 20                	jne    19f0 <__rt_poll+0xe0>
    19d0:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
    19d4:	74 1a                	je     19f0 <__rt_poll+0xe0>
    19d6:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 19dd <__rt_poll+0xcd>
    19dd:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 19e4 <__rt_poll+0xd4>
    19e4:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
    19eb:	00 00 00 00 00 
    19f0:	83 3a 00             	cmpl   $0x0,(%rdx)
    19f3:	75 cb                	jne    19c0 <__rt_poll+0xb0>
    19f5:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
    19f9:	74 c5                	je     19c0 <__rt_poll+0xb0>
    19fb:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1a02 <__rt_poll+0xf2>
    1a02:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1a09 <__rt_poll+0xf9>
    1a09:	eb b5                	jmp    19c0 <__rt_poll+0xb0>
    1a0b:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    1a0f:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
    1a15:	c5 fc 11 44 24 58    	vmovups %ymm0,0x58(%rsp)
    1a1b:	c5 fc 11 44 24 38    	vmovups %ymm0,0x38(%rsp)
    1a21:	c5 fc 11 44 24 18    	vmovups %ymm0,0x18(%rsp)
    1a27:	48 c7 44 24 10 01 00 	movq   $0x1,0x10(%rsp)
    1a2e:	00 00 
    1a30:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    1a34:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
    1a39:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
    1a3e:	49 89 e0             	mov    %rsp,%r8
    1a41:	bf 01 00 00 00       	mov    $0x1,%edi
    1a46:	31 d2                	xor    %edx,%edx
    1a48:	31 c9                	xor    %ecx,%ecx
    1a4a:	c5 f8 77             	vzeroupper
    1a4d:	e8 00 00 00 00       	call   1a52 <__rt_poll+0x142>
    1a52:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
    1a57:	0f 84 35 ff ff ff    	je     1992 <__rt_poll+0x82>
    1a5d:	eb 0b                	jmp    1a6a <__rt_poll+0x15a>
    1a5f:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
    1a64:	0f 84 28 ff ff ff    	je     1992 <__rt_poll+0x82>
    1a6a:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1a71 <__rt_poll+0x161>
    1a71:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1a78 <__rt_poll+0x168>
    1a78:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1a7f <__rt_poll+0x16f>
    1a7f:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
    1a86:	c3                   	ret
    1a87:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
    1a8e:	00 00 

0000000000001a90 <__wait_for_event>:
    1a90:	e9 00 00 00 00       	jmp    1a95 <__wait_for_event+0x5>
    1a95:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
    1a9c:	00 00 00 00 

0000000000001aa0 <__print>:
    1aa0:	50                   	push   %rax
    1aa1:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 1aa8 <__print+0x8>
    1aa8:	48 8b 30             	mov    (%rax),%rsi
    1aab:	e8 00 00 00 00       	call   1ab0 <__print+0x10>
    1ab0:	b8 01 00 00 00       	mov    $0x1,%eax
    1ab5:	59                   	pop    %rcx
    1ab6:	c3                   	ret
    1ab7:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
    1abe:	00 00 

0000000000001ac0 <__print_int>:
    1ac0:	50                   	push   %rax
    1ac1:	48 89 fa             	mov    %rdi,%rdx
    1ac4:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 1acb <__print_int+0xb>
    1acb:	48 8b 38             	mov    (%rax),%rdi
    1ace:	be 00 00 00 00       	mov    $0x0,%esi
    1ad3:	31 c0                	xor    %eax,%eax
    1ad5:	e8 00 00 00 00       	call   1ada <__print_int+0x1a>
    1ada:	b8 01 00 00 00       	mov    $0x1,%eax
    1adf:	59                   	pop    %rcx
    1ae0:	c3                   	ret
    1ae1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1ae8:	0f 1f 84 00 00 00 00 
    1aef:	00 

0000000000001af0 <__print_float>:
    1af0:	50                   	push   %rax
    1af1:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 1af8 <__print_float+0x8>
    1af8:	48 8b 38             	mov    (%rax),%rdi
    1afb:	c5 fa 5a c0          	vcvtss2sd %xmm0,%xmm0,%xmm0
    1aff:	be 00 00 00 00       	mov    $0x0,%esi
    1b04:	b0 01                	mov    $0x1,%al
    1b06:	e8 00 00 00 00       	call   1b0b <__print_float+0x1b>
    1b0b:	b8 01 00 00 00       	mov    $0x1,%eax
    1b10:	59                   	pop    %rcx
    1b11:	c3                   	ret
    1b12:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1b19:	1f 84 00 00 00 00 00 

0000000000001b20 <__sqrtf>:
    1b20:	c5 f0 57 c9          	vxorps %xmm1,%xmm1,%xmm1
    1b24:	c5 f8 2e c1          	vucomiss %xmm1,%xmm0
    1b28:	0f 82 00 00 00 00    	jb     1b2e <__sqrtf+0xe>
    1b2e:	c5 fa 51 c0          	vsqrtss %xmm0,%xmm0,%xmm0
    1b32:	c3                   	ret
    1b33:	66 66 66 66 2e 0f 1f 	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1b3a:	84 00 00 00 00 00 

0000000000001b40 <__exit>:
    1b40:	50                   	push   %rax
    1b41:	31 ff                	xor    %edi,%edi
    1b43:	e8 00 00 00 00       	call   1b48 <__exit+0x8>
    1b48:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
    1b4f:	00 

0000000000001b50 <__read_stdin>:
    1b50:	48 89 f2             	mov    %rsi,%rdx
    1b53:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 1b5a <__read_stdin+0xa>
    1b5a:	48 8b 08             	mov    (%rax),%rcx
    1b5d:	be 01 00 00 00       	mov    $0x1,%esi
    1b62:	e9 00 00 00 00       	jmp    1b67 <__read_stdin+0x17>
    1b67:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
    1b6e:	00 00 

0000000000001b70 <__putchar>:
    1b70:	53                   	push   %rbx
    1b71:	48 89 fb             	mov    %rdi,%rbx
    1b74:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 1b7b <__putchar+0xb>
    1b7b:	48 8b 30             	mov    (%rax),%rsi
    1b7e:	e8 00 00 00 00       	call   1b83 <__putchar+0x13>
    1b83:	48 89 d8             	mov    %rbx,%rax
    1b86:	5b                   	pop    %rbx
    1b87:	c3                   	ret
    1b88:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
    1b8f:	00 

0000000000001b90 <brief_thread_pool_init>:
    1b90:	c3                   	ret
    1b91:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1b98:	0f 1f 84 00 00 00 00 
    1b9f:	00 

0000000000001ba0 <brief_barrier_release>:
    1ba0:	c3                   	ret
    1ba1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1ba8:	0f 1f 84 00 00 00 00 
    1baf:	00 

0000000000001bb0 <brief_barrier_wait>:
    1bb0:	c3                   	ret
    1bb1:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1bb8:	0f 1f 84 00 00 00 00 
    1bbf:	00 

0000000000001bc0 <brief_thread_pool_shutdown>:
    1bc0:	c3                   	ret
