
trophies/per-field-gep/nbody_newton.o:     file format elf64-x86-64


Disassembly of section .text:

0000000000000000 <simulate>:
       0:	48 81 ec e8 00 00 00 	sub    $0xe8,%rsp
       7:	c5 fa 10 6f 30       	vmovss 0x30(%rdi),%xmm5
       c:	c5 fa 10 47 48       	vmovss 0x48(%rdi),%xmm0
      11:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
      16:	c5 fa 10 5f 60       	vmovss 0x60(%rdi),%xmm3
      1b:	c5 f8 29 5c 24 b0    	vmovaps %xmm3,-0x50(%rsp)
      21:	c5 fa 10 67 78       	vmovss 0x78(%rdi),%xmm4
      26:	c5 fa 5c c3          	vsubss %xmm3,%xmm0,%xmm0
      2a:	c5 fa 59 d0          	vmulss %xmm0,%xmm0,%xmm2
      2e:	c4 e3 51 21 cb 10    	vinsertps $0x10,%xmm3,%xmm5,%xmm1
      34:	c5 78 28 c5          	vmovaps %xmm5,%xmm8
      38:	c5 f8 29 6c 24 20    	vmovaps %xmm5,0x20(%rsp)
      3e:	c4 e3 61 21 dc 10    	vinsertps $0x10,%xmm4,%xmm3,%xmm3
      44:	c5 f8 28 fc          	vmovaps %xmm4,%xmm7
      48:	c5 f0 5c cb          	vsubps %xmm3,%xmm1,%xmm1
      4c:	c5 f8 29 4c 24 70    	vmovaps %xmm1,0x70(%rsp)
      52:	c5 fb 10 4f 40       	vmovsd 0x40(%rdi),%xmm1
      57:	c5 f8 29 4c 24 40    	vmovaps %xmm1,0x40(%rsp)
      5d:	c5 fb 10 5f 58       	vmovsd 0x58(%rdi),%xmm3
      62:	c5 f8 29 5c 24 30    	vmovaps %xmm3,0x30(%rsp)
      68:	c5 f0 5c cb          	vsubps %xmm3,%xmm1,%xmm1
      6c:	c5 f2 59 d9          	vmulss %xmm1,%xmm1,%xmm3
      70:	c5 e2 58 d2          	vaddss %xmm2,%xmm3,%xmm2
      74:	c5 fa 16 d9          	vmovshdup %xmm1,%xmm3
      78:	c5 e2 59 e3          	vmulss %xmm3,%xmm3,%xmm4
      7c:	c5 da 58 d2          	vaddss %xmm2,%xmm4,%xmm2
      80:	c5 7a 10 35 00 00 00 	vmovss 0x0(%rip),%xmm14        # 88 <simulate+0x88>
      87:	00 
      88:	c5 8a 59 e2          	vmulss %xmm2,%xmm14,%xmm4
      8c:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # 94 <simulate+0x94>
      93:	00 
      94:	c5 da 58 e5          	vaddss %xmm5,%xmm4,%xmm4
      98:	c5 78 28 fd          	vmovaps %xmm5,%xmm15
      9c:	c5 ea 5e ec          	vdivss %xmm4,%xmm2,%xmm5
      a0:	c5 d2 58 e4          	vaddss %xmm4,%xmm5,%xmm4
      a4:	c5 fa 10 35 00 00 00 	vmovss 0x0(%rip),%xmm6        # ac <simulate+0xac>
      ab:	00 
      ac:	c5 da 59 e6          	vmulss %xmm6,%xmm4,%xmm4
      b0:	c5 ea 5e ec          	vdivss %xmm4,%xmm2,%xmm5
      b4:	c5 d2 58 e4          	vaddss %xmm4,%xmm5,%xmm4
      b8:	c5 da 59 e6          	vmulss %xmm6,%xmm4,%xmm4
      bc:	c5 ea 5e ec          	vdivss %xmm4,%xmm2,%xmm5
      c0:	c5 d2 58 e4          	vaddss %xmm4,%xmm5,%xmm4
      c4:	c5 da 59 e6          	vmulss %xmm6,%xmm4,%xmm4
      c8:	c5 ea 5e ec          	vdivss %xmm4,%xmm2,%xmm5
      cc:	c5 d2 58 e4          	vaddss %xmm4,%xmm5,%xmm4
      d0:	c5 ea 59 d6          	vmulss %xmm6,%xmm2,%xmm2
      d4:	c5 ea 59 d4          	vmulss %xmm4,%xmm2,%xmm2
      d8:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # e0 <simulate+0xe0>
      df:	00 
      e0:	c5 d2 5e e2          	vdivss %xmm2,%xmm5,%xmm4
      e4:	c5 78 28 e5          	vmovaps %xmm5,%xmm12
      e8:	c5 da 59 15 00 00 00 	vmulss 0x0(%rip),%xmm4,%xmm2        # f0 <simulate+0xf0>
      ef:	00 
      f0:	c5 fa 12 ea          	vmovsldup %xmm2,%xmm5
      f4:	c5 d0 59 e9          	vmulps %xmm1,%xmm5,%xmm5
      f8:	c5 f8 29 6c 24 f0    	vmovaps %xmm5,-0x10(%rsp)
      fe:	c5 ea 59 d0          	vmulss %xmm0,%xmm2,%xmm2
     102:	c5 fa 11 54 24 8c    	vmovss %xmm2,-0x74(%rsp)
     108:	c5 da 59 25 00 00 00 	vmulss 0x0(%rip),%xmm4,%xmm4        # 110 <simulate+0x110>
     10f:	00 
     110:	c5 da 59 c9          	vmulss %xmm1,%xmm4,%xmm1
     114:	c5 fa 11 4c 24 90    	vmovss %xmm1,-0x70(%rsp)
     11a:	c5 da 59 cb          	vmulss %xmm3,%xmm4,%xmm1
     11e:	c5 da 59 c0          	vmulss %xmm0,%xmm4,%xmm0
     122:	c4 e3 71 21 c0 10    	vinsertps $0x10,%xmm0,%xmm1,%xmm0
     128:	c5 f8 29 84 24 80 00 	vmovaps %xmm0,0x80(%rsp)
     12f:	00 00 
     131:	c5 fb 10 4f 70       	vmovsd 0x70(%rdi),%xmm1
     136:	c5 7b 12 57 28       	vmovddup 0x28(%rdi),%xmm10
     13b:	c5 a8 5c d9          	vsubps %xmm1,%xmm10,%xmm3
     13f:	c5 78 28 e9          	vmovaps %xmm1,%xmm13
     143:	c5 e0 59 cb          	vmulps %xmm3,%xmm3,%xmm1
     147:	c5 fa 16 e1          	vmovshdup %xmm1,%xmm4
     14b:	c5 da 58 c9          	vaddss %xmm1,%xmm4,%xmm1
     14f:	c5 f8 28 d7          	vmovaps %xmm7,%xmm2
     153:	c5 f8 29 bc 24 a0 00 	vmovaps %xmm7,0xa0(%rsp)
     15a:	00 00 
     15c:	c5 3a 5c c7          	vsubss %xmm7,%xmm8,%xmm8
     160:	c4 c1 3a 59 e0       	vmulss %xmm8,%xmm8,%xmm4
     165:	c5 f2 58 cc          	vaddss %xmm4,%xmm1,%xmm1
     169:	c5 8a 59 e1          	vmulss %xmm1,%xmm14,%xmm4
     16d:	c5 82 58 e4          	vaddss %xmm4,%xmm15,%xmm4
     171:	c5 f2 5e ec          	vdivss %xmm4,%xmm1,%xmm5
     175:	c5 d2 58 e4          	vaddss %xmm4,%xmm5,%xmm4
     179:	c5 da 59 e6          	vmulss %xmm6,%xmm4,%xmm4
     17d:	c5 f2 5e ec          	vdivss %xmm4,%xmm1,%xmm5
     181:	c5 d2 58 e4          	vaddss %xmm4,%xmm5,%xmm4
     185:	c5 da 59 e6          	vmulss %xmm6,%xmm4,%xmm4
     189:	c5 f2 5e ec          	vdivss %xmm4,%xmm1,%xmm5
     18d:	c5 d2 58 e4          	vaddss %xmm4,%xmm5,%xmm4
     191:	c5 da 59 e6          	vmulss %xmm6,%xmm4,%xmm4
     195:	c5 f2 5e ec          	vdivss %xmm4,%xmm1,%xmm5
     199:	c5 d2 58 e4          	vaddss %xmm4,%xmm5,%xmm4
     19d:	c5 f2 59 ce          	vmulss %xmm6,%xmm1,%xmm1
     1a1:	c5 f2 59 cc          	vmulss %xmm4,%xmm1,%xmm1
     1a5:	c5 9a 5e e1          	vdivss %xmm1,%xmm12,%xmm4
     1a9:	c4 41 78 28 dc       	vmovaps %xmm12,%xmm11
     1ae:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # 1b6 <simulate+0x1b6>
     1b5:	00 
     1b6:	c5 5a 59 cd          	vmulss %xmm5,%xmm4,%xmm9
     1ba:	c4 c1 7a 12 c9       	vmovsldup %xmm9,%xmm1
     1bf:	c5 f0 59 c3          	vmulps %xmm3,%xmm1,%xmm0
     1c3:	c5 f8 29 44 24 a0    	vmovaps %xmm0,-0x60(%rsp)
     1c9:	c5 5a 59 25 00 00 00 	vmulss 0x0(%rip),%xmm4,%xmm12        # 1d1 <simulate+0x1d1>
     1d0:	00 
     1d1:	c4 c1 7a 12 e4       	vmovsldup %xmm12,%xmm4
     1d6:	c5 d8 59 fb          	vmulps %xmm3,%xmm4,%xmm7
     1da:	c4 c1 32 59 e0       	vmulss %xmm8,%xmm9,%xmm4
     1df:	c4 c1 1a 59 d8       	vmulss %xmm8,%xmm12,%xmm3
     1e4:	c4 e2 79 18 47 18    	vbroadcastss 0x18(%rdi),%xmm0
     1ea:	c5 f8 29 44 24 10    	vmovaps %xmm0,0x10(%rsp)
     1f0:	c5 7a 5c c2          	vsubss %xmm2,%xmm0,%xmm8
     1f4:	c4 41 3a 59 c8       	vmulss %xmm8,%xmm8,%xmm9
     1f9:	c5 fb 10 47 10       	vmovsd 0x10(%rdi),%xmm0
     1fe:	c5 f8 29 84 24 c0 00 	vmovaps %xmm0,0xc0(%rsp)
     205:	00 00 
     207:	c4 41 78 5c e5       	vsubps %xmm13,%xmm0,%xmm12
     20c:	c5 78 29 e8          	vmovaps %xmm13,%xmm0
     210:	c5 78 29 ac 24 90 00 	vmovaps %xmm13,0x90(%rsp)
     217:	00 00 
     219:	c4 41 1a 59 fc       	vmulss %xmm12,%xmm12,%xmm15
     21e:	c4 41 02 58 c9       	vaddss %xmm9,%xmm15,%xmm9
     223:	c4 41 7a 16 fc       	vmovshdup %xmm12,%xmm15
     228:	c4 c1 02 59 d7       	vmulss %xmm15,%xmm15,%xmm2
     22d:	c5 b2 58 d2          	vaddss %xmm2,%xmm9,%xmm2
     231:	c5 0a 59 ca          	vmulss %xmm2,%xmm14,%xmm9
     235:	c5 32 58 0d 00 00 00 	vaddss 0x0(%rip),%xmm9,%xmm9        # 23d <simulate+0x23d>
     23c:	00 
     23d:	c4 41 6a 5e e9       	vdivss %xmm9,%xmm2,%xmm13
     242:	c4 41 12 58 c9       	vaddss %xmm9,%xmm13,%xmm9
     247:	c5 32 59 ce          	vmulss %xmm6,%xmm9,%xmm9
     24b:	c4 41 6a 5e e9       	vdivss %xmm9,%xmm2,%xmm13
     250:	c4 41 12 58 c9       	vaddss %xmm9,%xmm13,%xmm9
     255:	c5 32 59 ce          	vmulss %xmm6,%xmm9,%xmm9
     259:	c4 41 6a 5e e9       	vdivss %xmm9,%xmm2,%xmm13
     25e:	c4 41 12 58 c9       	vaddss %xmm9,%xmm13,%xmm9
     263:	c5 32 59 ce          	vmulss %xmm6,%xmm9,%xmm9
     267:	c4 41 6a 5e e9       	vdivss %xmm9,%xmm2,%xmm13
     26c:	c4 41 12 58 c9       	vaddss %xmm9,%xmm13,%xmm9
     271:	c5 ea 59 d6          	vmulss %xmm6,%xmm2,%xmm2
     275:	c5 b2 59 d2          	vmulss %xmm2,%xmm9,%xmm2
     279:	c5 a2 5e d2          	vdivss %xmm2,%xmm11,%xmm2
     27d:	c5 6a 59 cd          	vmulss %xmm5,%xmm2,%xmm9
     281:	c4 41 32 59 ef       	vmulss %xmm15,%xmm9,%xmm13
     286:	c4 41 32 59 f8       	vmulss %xmm8,%xmm9,%xmm15
     28b:	c4 c3 11 21 ef 10    	vinsertps $0x10,%xmm15,%xmm13,%xmm5
     291:	c5 f8 29 6c 24 e0    	vmovaps %xmm5,-0x20(%rsp)
     297:	c4 c1 32 59 cc       	vmulss %xmm12,%xmm9,%xmm1
     29c:	c5 fa 11 4c 24 d0    	vmovss %xmm1,-0x30(%rsp)
     2a2:	c5 ea 59 15 00 00 00 	vmulss 0x0(%rip),%xmm2,%xmm2        # 2aa <simulate+0x2aa>
     2a9:	00 
     2aa:	c5 7a 12 ca          	vmovsldup %xmm2,%xmm9
     2ae:	c4 41 30 59 cc       	vmulps %xmm12,%xmm9,%xmm9
     2b3:	c5 7b 10 67 7c       	vmovsd 0x7c(%rdi),%xmm12
     2b8:	c4 41 18 58 c9       	vaddps %xmm9,%xmm12,%xmm9
     2bd:	c5 b0 58 ff          	vaddps %xmm7,%xmm9,%xmm7
     2c1:	c5 ba 59 d2          	vmulss %xmm2,%xmm8,%xmm2
     2c5:	c5 ea 58 97 84 00 00 	vaddss 0x84(%rdi),%xmm2,%xmm2
     2cc:	00 
     2cd:	c5 7b 10 47 68       	vmovsd 0x68(%rdi),%xmm8
     2d2:	c5 b8 16 cf          	vmovlhps %xmm7,%xmm8,%xmm1
     2d6:	c5 f8 29 4c 24 50    	vmovaps %xmm1,0x50(%rsp)
     2dc:	c5 ea 58 db          	vaddss %xmm3,%xmm2,%xmm3
     2e0:	c5 28 5c 4c 24 40    	vsubps 0x40(%rsp),%xmm10,%xmm9
     2e6:	c4 c1 30 59 d1       	vmulps %xmm9,%xmm9,%xmm2
     2eb:	c5 fa 16 fa          	vmovshdup %xmm2,%xmm7
     2ef:	c5 c2 58 d2          	vaddss %xmm2,%xmm7,%xmm2
     2f3:	c5 f8 28 6c 24 20    	vmovaps 0x20(%rsp),%xmm5
     2f9:	c5 78 28 04 24       	vmovaps (%rsp),%xmm8
     2fe:	c4 41 52 5c e0       	vsubss %xmm8,%xmm5,%xmm12
     303:	c4 c1 1a 59 fc       	vmulss %xmm12,%xmm12,%xmm7
     308:	c5 ea 58 d7          	vaddss %xmm7,%xmm2,%xmm2
     30c:	c5 8a 59 fa          	vmulss %xmm2,%xmm14,%xmm7
     310:	c5 c2 58 3d 00 00 00 	vaddss 0x0(%rip),%xmm7,%xmm7        # 318 <simulate+0x318>
     317:	00 
     318:	c5 6a 5e ef          	vdivss %xmm7,%xmm2,%xmm13
     31c:	c5 92 58 ff          	vaddss %xmm7,%xmm13,%xmm7
     320:	c5 c2 59 fe          	vmulss %xmm6,%xmm7,%xmm7
     324:	c5 6a 5e ef          	vdivss %xmm7,%xmm2,%xmm13
     328:	c5 92 58 ff          	vaddss %xmm7,%xmm13,%xmm7
     32c:	c5 c2 59 fe          	vmulss %xmm6,%xmm7,%xmm7
     330:	c5 6a 5e ef          	vdivss %xmm7,%xmm2,%xmm13
     334:	c5 92 58 ff          	vaddss %xmm7,%xmm13,%xmm7
     338:	c5 c2 59 fe          	vmulss %xmm6,%xmm7,%xmm7
     33c:	c5 6a 5e ef          	vdivss %xmm7,%xmm2,%xmm13
     340:	c5 92 58 ff          	vaddss %xmm7,%xmm13,%xmm7
     344:	c5 ea 59 d6          	vmulss %xmm6,%xmm2,%xmm2
     348:	c5 ea 59 d7          	vmulss %xmm7,%xmm2,%xmm2
     34c:	c5 a2 5e d2          	vdivss %xmm2,%xmm11,%xmm2
     350:	c5 6a 59 2d 00 00 00 	vmulss 0x0(%rip),%xmm2,%xmm13        # 358 <simulate+0x358>
     357:	00 
     358:	c4 c1 12 59 fc       	vmulss %xmm12,%xmm13,%xmm7
     35d:	c5 42 58 dc          	vaddss %xmm4,%xmm7,%xmm11
     361:	c5 ea 59 0d 00 00 00 	vmulss 0x0(%rip),%xmm2,%xmm1        # 369 <simulate+0x369>
     368:	00 
     369:	c5 fa 12 d1          	vmovsldup %xmm1,%xmm2
     36d:	c5 b0 59 d2          	vmulps %xmm2,%xmm9,%xmm2
     371:	c5 fb 10 67 4c       	vmovsd 0x4c(%rdi),%xmm4
     376:	c5 d8 58 d2          	vaddps %xmm2,%xmm4,%xmm2
     37a:	c5 f8 29 94 24 d0 00 	vmovaps %xmm2,0xd0(%rsp)
     381:	00 00 
     383:	c5 9a 59 c9          	vmulss %xmm1,%xmm12,%xmm1
     387:	c5 fa 11 4c 24 cc    	vmovss %xmm1,-0x34(%rsp)
     38d:	c4 c1 7a 12 cd       	vmovsldup %xmm13,%xmm1
     392:	c5 b0 59 c9          	vmulps %xmm1,%xmm9,%xmm1
     396:	c5 f0 58 7c 24 a0    	vaddps -0x60(%rsp),%xmm1,%xmm7
     39c:	c5 f8 28 4c 24 30    	vmovaps 0x30(%rsp),%xmm1
     3a2:	c5 28 5c c9          	vsubps %xmm1,%xmm10,%xmm9
     3a6:	c5 f0 5c c8          	vsubps %xmm0,%xmm1,%xmm1
     3aa:	c4 e3 31 21 c1 1c    	vinsertps $0x1c,%xmm1,%xmm9,%xmm0
     3b0:	c5 f8 59 c0          	vmulps %xmm0,%xmm0,%xmm0
     3b4:	c5 78 28 54 24 70    	vmovaps 0x70(%rsp),%xmm10
     3ba:	c4 41 28 59 e2       	vmulps %xmm10,%xmm10,%xmm12
     3bf:	c5 98 58 c0          	vaddps %xmm0,%xmm12,%xmm0
     3c3:	c4 41 7a 16 e1       	vmovshdup %xmm9,%xmm12
     3c8:	c4 63 19 0c e9 02    	vblendps $0x2,%xmm1,%xmm12,%xmm13
     3ce:	c4 41 10 59 ed       	vmulps %xmm13,%xmm13,%xmm13
     3d3:	c5 10 58 f8          	vaddps %xmm0,%xmm13,%xmm15
     3d7:	c5 80 59 05 00 00 00 	vmulps 0x0(%rip),%xmm15,%xmm0        # 3df <simulate+0x3df>
     3de:	00 
     3df:	c5 f8 58 05 00 00 00 	vaddps 0x0(%rip),%xmm0,%xmm0        # 3e7 <simulate+0x3e7>
     3e6:	00 
     3e7:	c5 78 53 e8          	vrcpps %xmm0,%xmm13
     3eb:	c4 c1 00 59 f5       	vmulps %xmm13,%xmm15,%xmm6
     3f0:	c5 f8 59 d6          	vmulps %xmm6,%xmm0,%xmm2
     3f4:	c5 80 5c d2          	vsubps %xmm2,%xmm15,%xmm2
     3f8:	c5 90 59 d2          	vmulps %xmm2,%xmm13,%xmm2
     3fc:	c5 c8 58 c0          	vaddps %xmm0,%xmm6,%xmm0
     400:	c5 f8 58 d2          	vaddps %xmm2,%xmm0,%xmm2
     404:	c4 e2 79 18 05 00 00 	vbroadcastss 0x0(%rip),%xmm0        # 40d <simulate+0x40d>
     40b:	00 00 
     40d:	c5 e8 59 d0          	vmulps %xmm0,%xmm2,%xmm2
     411:	c5 f8 53 f2          	vrcpps %xmm2,%xmm6
     415:	c5 00 59 ee          	vmulps %xmm6,%xmm15,%xmm13
     419:	c5 90 59 e2          	vmulps %xmm2,%xmm13,%xmm4
     41d:	c5 80 5c e4          	vsubps %xmm4,%xmm15,%xmm4
     421:	c5 c8 59 e4          	vmulps %xmm4,%xmm6,%xmm4
     425:	c5 90 58 d2          	vaddps %xmm2,%xmm13,%xmm2
     429:	c5 e8 58 d4          	vaddps %xmm4,%xmm2,%xmm2
     42d:	c5 e8 59 d0          	vmulps %xmm0,%xmm2,%xmm2
     431:	c5 f8 53 e2          	vrcpps %xmm2,%xmm4
     435:	c5 80 59 f4          	vmulps %xmm4,%xmm15,%xmm6
     439:	c5 68 59 ee          	vmulps %xmm6,%xmm2,%xmm13
     43d:	c4 41 00 5c ed       	vsubps %xmm13,%xmm15,%xmm13
     442:	c5 90 59 e4          	vmulps %xmm4,%xmm13,%xmm4
     446:	c5 c8 58 d2          	vaddps %xmm2,%xmm6,%xmm2
     44a:	c5 e8 58 d4          	vaddps %xmm4,%xmm2,%xmm2
     44e:	c5 e8 59 d0          	vmulps %xmm0,%xmm2,%xmm2
     452:	c5 f8 53 e2          	vrcpps %xmm2,%xmm4
     456:	c5 80 59 f4          	vmulps %xmm4,%xmm15,%xmm6
     45a:	c5 68 59 ee          	vmulps %xmm6,%xmm2,%xmm13
     45e:	c4 41 00 5c ed       	vsubps %xmm13,%xmm15,%xmm13
     463:	c5 90 59 e4          	vmulps %xmm4,%xmm13,%xmm4
     467:	c5 c8 58 d2          	vaddps %xmm2,%xmm6,%xmm2
     46b:	c5 e8 58 d4          	vaddps %xmm4,%xmm2,%xmm2
     46f:	c5 80 59 e0          	vmulps %xmm0,%xmm15,%xmm4
     473:	c5 d8 59 d2          	vmulps %xmm2,%xmm4,%xmm2
     477:	c5 f8 53 e2          	vrcpps %xmm2,%xmm4
     47b:	c4 e2 79 18 05 00 00 	vbroadcastss 0x0(%rip),%xmm0        # 484 <simulate+0x484>
     482:	00 00 
     484:	c5 d8 59 f0          	vmulps %xmm0,%xmm4,%xmm6
     488:	c5 e8 59 d6          	vmulps %xmm6,%xmm2,%xmm2
     48c:	c5 f8 5c d2          	vsubps %xmm2,%xmm0,%xmm2
     490:	c5 d8 59 d2          	vmulps %xmm2,%xmm4,%xmm2
     494:	c5 c8 58 d2          	vaddps %xmm2,%xmm6,%xmm2
     498:	c5 fa 16 e2          	vmovshdup %xmm2,%xmm4
     49c:	c5 da 59 25 00 00 00 	vmulss 0x0(%rip),%xmm4,%xmm4        # 4a4 <simulate+0x4a4>
     4a3:	00 
     4a4:	c5 fa 12 f4          	vmovsldup %xmm4,%xmm6
     4a8:	c5 78 29 d0          	vmovaps %xmm10,%xmm0
     4ac:	c4 63 29 21 e9 4c    	vinsertps $0x4c,%xmm1,%xmm10,%xmm13
     4b2:	c5 90 59 f6          	vmulps %xmm6,%xmm13,%xmm6
     4b6:	c5 f8 29 74 24 a0    	vmovaps %xmm6,-0x60(%rsp)
     4bc:	c4 c1 7a 16 f2       	vmovshdup %xmm10,%xmm6
     4c1:	c4 41 78 28 f2       	vmovaps %xmm10,%xmm14
     4c6:	c5 68 59 2d 00 00 00 	vmulps 0x0(%rip),%xmm2,%xmm13        # 4ce <simulate+0x4ce>
     4cd:	00 
     4ce:	c4 41 7a 16 d5       	vmovshdup %xmm13,%xmm10
     4d3:	c5 aa 59 f6          	vmulss %xmm6,%xmm10,%xmm6
     4d7:	c5 e2 58 c6          	vaddss %xmm6,%xmm3,%xmm0
     4db:	c5 fa 11 84 24 b0 00 	vmovss %xmm0,0xb0(%rsp)
     4e2:	00 00 
     4e4:	c5 ea 59 15 00 00 00 	vmulss 0x0(%rip),%xmm2,%xmm2        # 4ec <simulate+0x4ec>
     4eb:	00 
     4ec:	c5 fa 12 da          	vmovsldup %xmm2,%xmm3
     4f0:	c5 b0 59 db          	vmulps %xmm3,%xmm9,%xmm3
     4f4:	c5 c0 58 c3          	vaddps %xmm3,%xmm7,%xmm0
     4f8:	c5 f8 29 44 24 60    	vmovaps %xmm0,0x60(%rsp)
     4fe:	c5 8a 59 d2          	vmulss %xmm2,%xmm14,%xmm2
     502:	c5 22 58 fa          	vaddss %xmm2,%xmm11,%xmm15
     506:	c4 c1 12 59 d1       	vmulss %xmm9,%xmm13,%xmm2
     50b:	c5 ea 58 44 24 90    	vaddss -0x70(%rsp),%xmm2,%xmm0
     511:	c5 fa 11 44 24 90    	vmovss %xmm0,-0x70(%rsp)
     517:	c4 c1 18 14 d6       	vunpcklps %xmm14,%xmm12,%xmm2
     51c:	c5 da 59 c1          	vmulss %xmm1,%xmm4,%xmm0
     520:	c5 fa 11 44 24 70    	vmovss %xmm0,0x70(%rsp)
     526:	c5 e8 16 c9          	vmovlhps %xmm1,%xmm2,%xmm1
     52a:	c4 c1 10 c6 d5 50    	vshufps $0x50,%xmm13,%xmm13,%xmm2
     530:	c5 e8 59 c9          	vmulps %xmm1,%xmm2,%xmm1
     534:	c5 f0 58 44 24 50    	vaddps 0x50(%rsp),%xmm1,%xmm0
     53a:	c5 f8 29 44 24 50    	vmovaps %xmm0,0x50(%rsp)
     540:	c5 78 28 b4 24 c0 00 	vmovaps 0xc0(%rsp),%xmm14
     547:	00 00 
     549:	c5 8a 5c 5f 28       	vsubss 0x28(%rdi),%xmm14,%xmm3
     54e:	c4 41 78 28 d8       	vmovaps %xmm8,%xmm11
     553:	c4 c3 51 21 c8 10    	vinsertps $0x10,%xmm8,%xmm5,%xmm1
     559:	c5 78 28 6c 24 10    	vmovaps 0x10(%rsp),%xmm13
     55f:	c5 90 5c f9          	vsubps %xmm1,%xmm13,%xmm7
     563:	c5 78 28 64 24 40    	vmovaps 0x40(%rsp),%xmm12
     569:	c4 c1 08 5c cc       	vsubps %xmm12,%xmm14,%xmm1
     56e:	c5 fa 12 d1          	vmovsldup %xmm1,%xmm2
     572:	c4 e3 69 0c d3 01    	vblendps $0x1,%xmm3,%xmm2,%xmm2
     578:	c5 e8 59 d2          	vmulps %xmm2,%xmm2,%xmm2
     57c:	c5 c0 59 e7          	vmulps %xmm7,%xmm7,%xmm4
     580:	c5 e8 58 d4          	vaddps %xmm4,%xmm2,%xmm2
     584:	c4 c1 7a 16 e6       	vmovshdup %xmm14,%xmm4
     589:	c5 5a 5c 4f 2c       	vsubss 0x2c(%rdi),%xmm4,%xmm9
     58e:	c4 c3 71 0c e1 01    	vblendps $0x1,%xmm9,%xmm1,%xmm4
     594:	c5 d8 59 e4          	vmulps %xmm4,%xmm4,%xmm4
     598:	c5 58 58 c2          	vaddps %xmm2,%xmm4,%xmm8
     59c:	c5 b8 59 15 00 00 00 	vmulps 0x0(%rip),%xmm8,%xmm2        # 5a4 <simulate+0x5a4>
     5a3:	00 
     5a4:	c5 e8 58 15 00 00 00 	vaddps 0x0(%rip),%xmm2,%xmm2        # 5ac <simulate+0x5ac>
     5ab:	00 
     5ac:	c5 f8 53 e2          	vrcpps %xmm2,%xmm4
     5b0:	c5 b8 59 f4          	vmulps %xmm4,%xmm8,%xmm6
     5b4:	c5 68 59 d6          	vmulps %xmm6,%xmm2,%xmm10
     5b8:	c4 41 38 5c d2       	vsubps %xmm10,%xmm8,%xmm10
     5bd:	c5 a8 59 e4          	vmulps %xmm4,%xmm10,%xmm4
     5c1:	c5 c8 58 d2          	vaddps %xmm2,%xmm6,%xmm2
     5c5:	c5 e8 58 d4          	vaddps %xmm4,%xmm2,%xmm2
     5c9:	c4 e2 79 18 05 00 00 	vbroadcastss 0x0(%rip),%xmm0        # 5d2 <simulate+0x5d2>
     5d0:	00 00 
     5d2:	c5 e8 59 d0          	vmulps %xmm0,%xmm2,%xmm2
     5d6:	c5 f8 53 e2          	vrcpps %xmm2,%xmm4
     5da:	c5 b8 59 f4          	vmulps %xmm4,%xmm8,%xmm6
     5de:	c5 68 59 d6          	vmulps %xmm6,%xmm2,%xmm10
     5e2:	c4 41 38 5c d2       	vsubps %xmm10,%xmm8,%xmm10
     5e7:	c5 a8 59 e4          	vmulps %xmm4,%xmm10,%xmm4
     5eb:	c5 c8 58 d2          	vaddps %xmm2,%xmm6,%xmm2
     5ef:	c5 e8 58 d4          	vaddps %xmm4,%xmm2,%xmm2
     5f3:	c5 e8 59 d0          	vmulps %xmm0,%xmm2,%xmm2
     5f7:	c5 f8 53 e2          	vrcpps %xmm2,%xmm4
     5fb:	c5 b8 59 f4          	vmulps %xmm4,%xmm8,%xmm6
     5ff:	c5 68 59 d6          	vmulps %xmm6,%xmm2,%xmm10
     603:	c4 41 38 5c d2       	vsubps %xmm10,%xmm8,%xmm10
     608:	c5 a8 59 e4          	vmulps %xmm4,%xmm10,%xmm4
     60c:	c5 c8 58 d2          	vaddps %xmm2,%xmm6,%xmm2
     610:	c5 e8 58 d4          	vaddps %xmm4,%xmm2,%xmm2
     614:	c5 e8 59 d0          	vmulps %xmm0,%xmm2,%xmm2
     618:	c5 f8 53 e2          	vrcpps %xmm2,%xmm4
     61c:	c5 b8 59 f4          	vmulps %xmm4,%xmm8,%xmm6
     620:	c5 68 59 d6          	vmulps %xmm6,%xmm2,%xmm10
     624:	c4 41 38 5c d2       	vsubps %xmm10,%xmm8,%xmm10
     629:	c5 a8 59 e4          	vmulps %xmm4,%xmm10,%xmm4
     62d:	c5 c8 58 d2          	vaddps %xmm2,%xmm6,%xmm2
     631:	c5 e8 58 d4          	vaddps %xmm4,%xmm2,%xmm2
     635:	c5 b8 59 e0          	vmulps %xmm0,%xmm8,%xmm4
     639:	c5 d8 59 d2          	vmulps %xmm2,%xmm4,%xmm2
     63d:	c5 f8 53 e2          	vrcpps %xmm2,%xmm4
     641:	c4 e2 79 18 05 00 00 	vbroadcastss 0x0(%rip),%xmm0        # 64a <simulate+0x64a>
     648:	00 00 
     64a:	c5 d8 59 f0          	vmulps %xmm0,%xmm4,%xmm6
     64e:	c5 e8 59 d6          	vmulps %xmm6,%xmm2,%xmm2
     652:	c5 f8 5c d2          	vsubps %xmm2,%xmm0,%xmm2
     656:	c5 78 28 c0          	vmovaps %xmm0,%xmm8
     65a:	c5 d8 59 d2          	vmulps %xmm2,%xmm4,%xmm2
     65e:	c5 c8 58 d2          	vaddps %xmm2,%xmm6,%xmm2
     662:	c5 fa 10 05 00 00 00 	vmovss 0x0(%rip),%xmm0        # 66a <simulate+0x66a>
     669:	00 
     66a:	c5 ea 59 e0          	vmulss %xmm0,%xmm2,%xmm4
     66e:	c5 da 59 f7          	vmulss %xmm7,%xmm4,%xmm6
     672:	c5 ca 58 77 3c       	vaddss 0x3c(%rdi),%xmm6,%xmm6
     677:	c5 68 59 15 00 00 00 	vmulps 0x0(%rip),%xmm2,%xmm10        # 67f <simulate+0x67f>
     67e:	00 
     67f:	c4 c1 4a 5c ef       	vsubss %xmm15,%xmm6,%xmm5
     684:	c5 fa 11 6c 24 c8    	vmovss %xmm5,-0x38(%rsp)
     68a:	c4 c1 7a 16 ea       	vmovshdup %xmm10,%xmm5
     68f:	c5 d2 59 e9          	vmulss %xmm1,%xmm5,%xmm5
     693:	c5 d2 58 6c 24 d0    	vaddss -0x30(%rsp),%xmm5,%xmm5
     699:	c5 fa 11 6c 24 c4    	vmovss %xmm5,-0x3c(%rsp)
     69f:	c5 fa 16 e9          	vmovshdup %xmm1,%xmm5
     6a3:	c5 d0 14 ef          	vunpcklps %xmm7,%xmm5,%xmm5
     6a7:	c4 e3 51 21 f3 20    	vinsertps $0x20,%xmm3,%xmm5,%xmm6
     6ad:	c5 aa 59 eb          	vmulss %xmm3,%xmm10,%xmm5
     6b1:	c4 c3 41 0c d9 01    	vblendps $0x1,%xmm9,%xmm7,%xmm3
     6b7:	c5 a8 59 db          	vmulps %xmm3,%xmm10,%xmm3
     6bb:	c4 e3 49 21 f4 30    	vinsertps $0x30,%xmm4,%xmm6,%xmm6
     6c1:	c5 a8 c6 e4 c1       	vshufps $0xc1,%xmm4,%xmm10,%xmm4
     6c6:	c4 c3 59 21 e1 30    	vinsertps $0x30,%xmm9,%xmm4,%xmm4
     6cc:	c5 c8 59 e4          	vmulps %xmm4,%xmm6,%xmm4
     6d0:	c5 e0 16 5f 34       	vmovhps 0x34(%rdi),%xmm3,%xmm3
     6d5:	c5 e0 58 dc          	vaddps %xmm4,%xmm3,%xmm3
     6d9:	c5 fa 16 d2          	vmovshdup %xmm2,%xmm2
     6dd:	c5 ea 59 d0          	vmulss %xmm0,%xmm2,%xmm2
     6e1:	c5 fa 12 e2          	vmovsldup %xmm2,%xmm4
     6e5:	c5 d8 59 c9          	vmulps %xmm1,%xmm4,%xmm1
     6e9:	c5 70 58 8c 24 d0 00 	vaddps 0xd0(%rsp),%xmm1,%xmm9
     6f0:	00 00 
     6f2:	c5 fa 16 cf          	vmovshdup %xmm7,%xmm1
     6f6:	c5 ea 59 c9          	vmulss %xmm1,%xmm2,%xmm1
     6fa:	c5 f2 58 4f 54       	vaddss 0x54(%rdi),%xmm1,%xmm1
     6ff:	c5 f2 58 64 24 cc    	vaddss -0x34(%rsp),%xmm1,%xmm4
     705:	c5 f8 28 44 24 30    	vmovaps 0x30(%rsp),%xmm0
     70b:	c5 fa 16 c8          	vmovshdup %xmm0,%xmm1
     70f:	c4 e3 71 21 4c 24 b0 	vinsertps $0x10,-0x50(%rsp),%xmm1,%xmm1
     716:	10 
     717:	c5 f1 14 8c 24 90 00 	vunpcklpd 0x90(%rsp),%xmm1,%xmm1
     71e:	00 00 
     720:	c4 41 78 28 fe       	vmovaps %xmm14,%xmm15
     725:	c4 c3 11 21 d6 4c    	vinsertps $0x4c,%xmm14,%xmm13,%xmm2
     72b:	c4 c1 68 16 d4       	vmovlhps %xmm12,%xmm2,%xmm2
     730:	c5 e8 5c d1          	vsubps %xmm1,%xmm2,%xmm2
     734:	c5 88 5c c8          	vsubps %xmm0,%xmm14,%xmm1
     738:	c5 f0 59 f1          	vmulps %xmm1,%xmm1,%xmm6
     73c:	c5 e8 59 fa          	vmulps %xmm2,%xmm2,%xmm7
     740:	c4 e3 49 21 f7 90    	vinsertps $0x90,%xmm7,%xmm6,%xmm6
     746:	c5 c0 c6 ff ec       	vshufps $0xec,%xmm7,%xmm7,%xmm7
     74b:	c5 c0 58 f6          	vaddps %xmm6,%xmm7,%xmm6
     74f:	c5 a2 5c bc 24 a0 00 	vsubss 0xa0(%rsp),%xmm11,%xmm7
     756:	00 00 
     758:	c5 7a 16 d2          	vmovshdup %xmm2,%xmm10
     75c:	c4 63 29 21 d7 10    	vinsertps $0x10,%xmm7,%xmm10,%xmm10
     762:	c4 41 28 59 d2       	vmulps %xmm10,%xmm10,%xmm10
     767:	c5 28 58 de          	vaddps %xmm6,%xmm10,%xmm11
     76b:	c5 a0 59 35 00 00 00 	vmulps 0x0(%rip),%xmm11,%xmm6        # 773 <simulate+0x773>
     772:	00 
     773:	c5 c8 58 35 00 00 00 	vaddps 0x0(%rip),%xmm6,%xmm6        # 77b <simulate+0x77b>
     77a:	00 
     77b:	c5 78 53 d6          	vrcpps %xmm6,%xmm10
     77f:	c4 41 20 59 e2       	vmulps %xmm10,%xmm11,%xmm12
     784:	c5 18 59 ee          	vmulps %xmm6,%xmm12,%xmm13
     788:	c4 41 20 5c ed       	vsubps %xmm13,%xmm11,%xmm13
     78d:	c4 41 28 59 d5       	vmulps %xmm13,%xmm10,%xmm10
     792:	c5 98 58 f6          	vaddps %xmm6,%xmm12,%xmm6
     796:	c5 a8 58 f6          	vaddps %xmm6,%xmm10,%xmm6
     79a:	c4 e2 79 18 05 00 00 	vbroadcastss 0x0(%rip),%xmm0        # 7a3 <simulate+0x7a3>
     7a1:	00 00 
     7a3:	c5 c8 59 f0          	vmulps %xmm0,%xmm6,%xmm6
     7a7:	c5 78 53 d6          	vrcpps %xmm6,%xmm10
     7ab:	c4 41 20 59 e2       	vmulps %xmm10,%xmm11,%xmm12
     7b0:	c5 18 59 ee          	vmulps %xmm6,%xmm12,%xmm13
     7b4:	c4 41 20 5c ed       	vsubps %xmm13,%xmm11,%xmm13
     7b9:	c4 41 28 59 d5       	vmulps %xmm13,%xmm10,%xmm10
     7be:	c5 98 58 f6          	vaddps %xmm6,%xmm12,%xmm6
     7c2:	c5 a8 58 f6          	vaddps %xmm6,%xmm10,%xmm6
     7c6:	c5 c8 59 f0          	vmulps %xmm0,%xmm6,%xmm6
     7ca:	c5 78 53 d6          	vrcpps %xmm6,%xmm10
     7ce:	c4 41 20 59 e2       	vmulps %xmm10,%xmm11,%xmm12
     7d3:	c5 18 59 ee          	vmulps %xmm6,%xmm12,%xmm13
     7d7:	c4 41 20 5c ed       	vsubps %xmm13,%xmm11,%xmm13
     7dc:	c4 41 28 59 d5       	vmulps %xmm13,%xmm10,%xmm10
     7e1:	c5 98 58 f6          	vaddps %xmm6,%xmm12,%xmm6
     7e5:	c5 a8 58 f6          	vaddps %xmm6,%xmm10,%xmm6
     7e9:	c5 c8 59 f0          	vmulps %xmm0,%xmm6,%xmm6
     7ed:	c5 78 53 d6          	vrcpps %xmm6,%xmm10
     7f1:	c4 41 20 59 e2       	vmulps %xmm10,%xmm11,%xmm12
     7f6:	c5 18 59 ee          	vmulps %xmm6,%xmm12,%xmm13
     7fa:	c4 41 20 5c ed       	vsubps %xmm13,%xmm11,%xmm13
     7ff:	c4 41 28 59 d5       	vmulps %xmm13,%xmm10,%xmm10
     804:	c5 98 58 f6          	vaddps %xmm6,%xmm12,%xmm6
     808:	c5 a8 58 f6          	vaddps %xmm6,%xmm10,%xmm6
     80c:	c5 a0 59 c0          	vmulps %xmm0,%xmm11,%xmm0
     810:	c5 f8 59 c6          	vmulps %xmm6,%xmm0,%xmm0
     814:	c5 f8 53 f0          	vrcpps %xmm0,%xmm6
     818:	c4 41 78 28 f0       	vmovaps %xmm8,%xmm14
     81d:	c5 38 59 d6          	vmulps %xmm6,%xmm8,%xmm10
     821:	c5 a8 59 c0          	vmulps %xmm0,%xmm10,%xmm0
     825:	c5 b8 5c c0          	vsubps %xmm0,%xmm8,%xmm0
     829:	c4 41 78 28 e8       	vmovaps %xmm8,%xmm13
     82e:	c5 c8 59 c0          	vmulps %xmm0,%xmm6,%xmm0
     832:	c5 a8 58 f0          	vaddps %xmm0,%xmm10,%xmm6
     836:	c5 fa 16 c6          	vmovshdup %xmm6,%xmm0
     83a:	c5 fa 59 05 00 00 00 	vmulss 0x0(%rip),%xmm0,%xmm0        # 842 <simulate+0x842>
     841:	00 
     842:	c5 7a 12 d0          	vmovsldup %xmm0,%xmm10
     846:	c5 69 c6 da 01       	vshufpd $0x1,%xmm2,%xmm2,%xmm11
     84b:	c4 41 28 59 d3       	vmulps %xmm11,%xmm10,%xmm10
     850:	c5 28 58 54 24 f0    	vaddps -0x10(%rsp),%xmm10,%xmm10
     856:	c4 41 30 5c f2       	vsubps %xmm10,%xmm9,%xmm14
     85b:	c5 78 29 74 24 d0    	vmovaps %xmm14,-0x30(%rsp)
     861:	c5 fa 59 c7          	vmulss %xmm7,%xmm0,%xmm0
     865:	c5 fa 58 44 24 8c    	vaddss -0x74(%rsp),%xmm0,%xmm0
     86b:	c5 48 59 15 00 00 00 	vmulps 0x0(%rip),%xmm6,%xmm10        # 873 <simulate+0x873>
     872:	00 
     873:	c5 5a 5c c8          	vsubss %xmm0,%xmm4,%xmm9
     877:	c5 7a 11 4c 24 f0    	vmovss %xmm9,-0x10(%rsp)
     87d:	c4 c1 7a 16 c2       	vmovshdup %xmm10,%xmm0
     882:	c5 fa 59 c7          	vmulss %xmm7,%xmm0,%xmm0
     886:	c5 fa 58 84 24 b0 00 	vaddss 0xb0(%rsp),%xmm0,%xmm0
     88d:	00 00 
     88f:	c5 fa 11 44 24 8c    	vmovss %xmm0,-0x74(%rsp)
     895:	c5 ca 59 35 00 00 00 	vmulss 0x0(%rip),%xmm6,%xmm6        # 89d <simulate+0x89d>
     89c:	00 
     89d:	c5 ca 59 f9          	vmulss %xmm1,%xmm6,%xmm7
     8a1:	c5 d2 58 ef          	vaddss %xmm7,%xmm5,%xmm5
     8a5:	c5 d2 58 6c 24 c4    	vaddss -0x3c(%rsp),%xmm5,%xmm5
     8ab:	c5 aa 59 c9          	vmulss %xmm1,%xmm10,%xmm1
     8af:	c5 f2 58 4f 64       	vaddss 0x64(%rdi),%xmm1,%xmm1
     8b4:	c5 72 58 64 24 90    	vaddss -0x70(%rsp),%xmm1,%xmm12
     8ba:	c4 c1 28 c6 ca 50    	vshufps $0x50,%xmm10,%xmm10,%xmm1
     8c0:	c5 f0 59 ca          	vmulps %xmm2,%xmm1,%xmm1
     8c4:	c5 f0 58 64 24 50    	vaddps 0x50(%rsp),%xmm1,%xmm4
     8ca:	c5 fa 10 4f 1c       	vmovss 0x1c(%rdi),%xmm1
     8cf:	c5 f2 5c fd          	vsubss %xmm5,%xmm1,%xmm7
     8d3:	c5 f8 29 7c 24 90    	vmovaps %xmm7,-0x70(%rsp)
     8d9:	c4 e3 69 21 ce 10    	vinsertps $0x10,%xmm6,%xmm2,%xmm1
     8df:	c4 e3 69 0c d6 01    	vblendps $0x1,%xmm6,%xmm2,%xmm2
     8e5:	c5 f0 59 ca          	vmulps %xmm2,%xmm1,%xmm1
     8e9:	c5 f1 14 4c 24 60    	vunpcklpd 0x60(%rsp),%xmm1,%xmm1
     8ef:	c5 e0 58 c1          	vaddps %xmm1,%xmm3,%xmm0
     8f3:	c5 60 5c c1          	vsubps %xmm1,%xmm3,%xmm8
     8f7:	c5 9a 5c 6c 24 70    	vsubss 0x70(%rsp),%xmm12,%xmm5
     8fd:	c5 f8 29 44 24 60    	vmovaps %xmm0,0x60(%rsp)
     903:	c5 f8 58 54 24 e0    	vaddps -0x20(%rsp),%xmm0,%xmm2
     909:	c4 e2 79 18 35 00 00 	vbroadcastss 0x0(%rip),%xmm6        # 912 <simulate+0x912>
     910:	00 00 
     912:	c5 b8 59 de          	vmulps %xmm6,%xmm8,%xmm3
     916:	c4 e3 61 0c da 03    	vblendps $0x3,%xmm2,%xmm3,%xmm3
     91c:	c5 78 10 57 20       	vmovups 0x20(%rdi),%xmm10
     921:	c5 a8 5c d3          	vsubps %xmm3,%xmm10,%xmm2
     925:	c5 a8 58 c3          	vaddps %xmm3,%xmm10,%xmm0
     929:	c5 f8 29 44 24 e0    	vmovaps %xmm0,-0x20(%rsp)
     92f:	c5 7a 12 d2          	vmovsldup %xmm2,%xmm10
     933:	c4 63 29 0c d7 01    	vblendps $0x1,%xmm7,%xmm10,%xmm10
     939:	c5 78 29 e8          	vmovaps %xmm13,%xmm0
     93d:	c4 41 28 59 d5       	vmulps %xmm13,%xmm10,%xmm10
     942:	c4 c1 28 58 cf       	vaddps %xmm15,%xmm10,%xmm1
     947:	c5 f8 29 4c 24 70    	vmovaps %xmm1,0x70(%rsp)
     94d:	c5 fa 16 ca          	vmovshdup %xmm2,%xmm1
     951:	c5 7a 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm13        # 959 <simulate+0x959>
     958:	00 
     959:	c5 f8 29 8c 24 b0 00 	vmovaps %xmm1,0xb0(%rsp)
     960:	00 00 
     962:	c5 12 59 d1          	vmulss %xmm1,%xmm13,%xmm10
     966:	c5 2a 58 7c 24 10    	vaddss 0x10(%rsp),%xmm10,%xmm15
     96c:	c5 7a 10 64 24 c8    	vmovss -0x38(%rsp),%xmm12
     972:	c4 41 1a 59 d5       	vmulss %xmm13,%xmm12,%xmm10
     977:	c5 2a 58 5c 24 20    	vaddss 0x20(%rsp),%xmm10,%xmm11
     97d:	c5 08 59 d0          	vmulps %xmm0,%xmm14,%xmm10
     981:	c5 f8 28 c8          	vmovaps %xmm0,%xmm1
     985:	c5 a8 58 5c 24 40    	vaddps 0x40(%rsp),%xmm10,%xmm3
     98b:	c4 41 32 59 d5       	vmulss %xmm13,%xmm9,%xmm10
     990:	c4 41 78 28 f5       	vmovaps %xmm13,%xmm14
     995:	c5 2a 58 0c 24       	vaddss (%rsp),%xmm10,%xmm9
     99a:	c5 58 58 94 24 80 00 	vaddps 0x80(%rsp),%xmm4,%xmm10
     9a1:	00 00 
     9a3:	c5 d8 59 f6          	vmulps %xmm6,%xmm4,%xmm6
     9a7:	c4 c3 49 0c f2 03    	vblendps $0x3,%xmm10,%xmm6,%xmm6
     9ad:	c5 f8 28 44 24 a0    	vmovaps -0x60(%rsp),%xmm0
     9b3:	c5 79 14 ac 24 90 00 	vunpcklpd 0x90(%rsp),%xmm0,%xmm13
     9ba:	00 00 
     9bc:	c4 41 48 5c d5       	vsubps %xmm13,%xmm6,%xmm10
     9c1:	c5 90 58 fe          	vaddps %xmm6,%xmm13,%xmm7
     9c5:	c4 c1 7a 12 f2       	vmovsldup %xmm10,%xmm6
     9ca:	c4 e3 49 0c f5 01    	vblendps $0x1,%xmm5,%xmm6,%xmm6
     9d0:	c5 c8 59 f1          	vmulps %xmm1,%xmm6,%xmm6
     9d4:	c5 c8 58 44 24 30    	vaddps 0x30(%rsp),%xmm6,%xmm0
     9da:	c4 c1 7a 16 ca       	vmovshdup %xmm10,%xmm1
     9df:	c5 f8 29 4c 24 50    	vmovaps %xmm1,0x50(%rsp)
     9e5:	c4 41 78 28 ee       	vmovaps %xmm14,%xmm13
     9ea:	c5 8a 59 f1          	vmulss %xmm1,%xmm14,%xmm6
     9ee:	c5 4a 58 74 24 b0    	vaddss -0x50(%rsp),%xmm6,%xmm14
     9f4:	c5 fa 10 4c 24 8c    	vmovss -0x74(%rsp),%xmm1
     9fa:	c5 92 59 f1          	vmulss %xmm1,%xmm13,%xmm6
     9fe:	c5 ca 58 b4 24 a0 00 	vaddss 0xa0(%rsp),%xmm6,%xmm6
     a05:	00 00 
     a07:	48 8b 47 08          	mov    0x8(%rdi),%rax
     a0b:	48 ff c0             	inc    %rax
     a0e:	48 3b 07             	cmp    (%rdi),%rax
     a11:	c5 78 28 6c 24 90    	vmovaps -0x70(%rsp),%xmm13
     a17:	c5 7a 11 6f 1c       	vmovss %xmm13,0x1c(%rdi)
     a1c:	c5 78 29 44 24 a0    	vmovaps %xmm8,-0x60(%rsp)
     a22:	c5 78 17 47 34       	vmovhps %xmm8,0x34(%rdi)
     a27:	c4 41 78 28 ec       	vmovaps %xmm12,%xmm13
     a2c:	c5 7a 11 67 3c       	vmovss %xmm12,0x3c(%rdi)
     a31:	c5 78 28 44 24 d0    	vmovaps -0x30(%rsp),%xmm8
     a37:	c5 78 13 47 4c       	vmovlps %xmm8,0x4c(%rdi)
     a3c:	c5 7a 10 44 24 f0    	vmovss -0x10(%rsp),%xmm8
     a42:	c5 7a 11 47 54       	vmovss %xmm8,0x54(%rdi)
     a47:	c5 f8 29 ac 24 80 00 	vmovaps %xmm5,0x80(%rsp)
     a4e:	00 00 
     a50:	c5 fa 11 6f 64       	vmovss %xmm5,0x64(%rdi)
     a55:	c5 f8 29 24 24       	vmovaps %xmm4,(%rsp)
     a5a:	c5 f8 17 67 7c       	vmovhps %xmm4,0x7c(%rdi)
     a5f:	c5 fa 11 8f 84 00 00 	vmovss %xmm1,0x84(%rdi)
     a66:	00 
     a67:	c4 63 69 0c 6c 24 e0 	vblendps $0xc,-0x20(%rsp),%xmm2,%xmm13
     a6e:	0c 
     a6f:	c5 f8 28 4c 24 70    	vmovaps 0x70(%rsp),%xmm1
     a75:	c5 f8 13 4f 10       	vmovlps %xmm1,0x10(%rdi)
     a7a:	c5 7a 11 7c 24 20    	vmovss %xmm15,0x20(%rsp)
     a80:	c5 7a 11 7f 18       	vmovss %xmm15,0x18(%rdi)
     a85:	c5 78 11 6f 20       	vmovups %xmm13,0x20(%rdi)
     a8a:	c5 7a 11 5c 24 b0    	vmovss %xmm11,-0x50(%rsp)
     a90:	c5 7a 11 5f 30       	vmovss %xmm11,0x30(%rdi)
     a95:	c5 f8 29 9c 24 a0 00 	vmovaps %xmm3,0xa0(%rsp)
     a9c:	00 00 
     a9e:	c5 f8 13 5f 40       	vmovlps %xmm3,0x40(%rdi)
     aa3:	c5 7a 11 4c 24 30    	vmovss %xmm9,0x30(%rsp)
     aa9:	c5 7a 11 4f 48       	vmovss %xmm9,0x48(%rdi)
     aae:	c5 f8 29 7c 24 10    	vmovaps %xmm7,0x10(%rsp)
     ab4:	c4 c3 41 0c d2 03    	vblendps $0x3,%xmm10,%xmm7,%xmm2
     aba:	c5 f8 29 84 24 90 00 	vmovaps %xmm0,0x90(%rsp)
     ac1:	00 00 
     ac3:	c5 f8 13 47 58       	vmovlps %xmm0,0x58(%rdi)
     ac8:	c5 7a 11 74 24 40    	vmovss %xmm14,0x40(%rsp)
     ace:	c5 7a 11 77 60       	vmovss %xmm14,0x60(%rdi)
     ad3:	c5 f8 11 57 68       	vmovups %xmm2,0x68(%rdi)
     ad8:	c5 fa 11 77 78       	vmovss %xmm6,0x78(%rdi)
     add:	48 89 47 08          	mov    %rax,0x8(%rdi)
     ae1:	0f 85 35 06 00 00    	jne    111c <simulate+0x111c>
     ae7:	c5 78 28 c1          	vmovaps %xmm1,%xmm8
     aeb:	c5 78 28 de          	vmovaps %xmm6,%xmm11
     aef:	c5 f8 28 44 24 a0    	vmovaps -0x60(%rsp),%xmm0
     af5:	c4 e3 79 0c 4c 24 60 	vblendps $0x3,0x60(%rsp),%xmm0,%xmm1
     afc:	03 
     afd:	c5 f8 28 44 24 90    	vmovaps -0x70(%rsp),%xmm0
     b03:	c5 fa 59 f0          	vmulss %xmm0,%xmm0,%xmm6
     b07:	c4 c1 12 59 fd       	vmulss %xmm13,%xmm13,%xmm7
     b0c:	c5 c2 58 f6          	vaddss %xmm6,%xmm7,%xmm6
     b10:	c5 f8 28 84 24 b0 00 	vmovaps 0xb0(%rsp),%xmm0
     b17:	00 00 
     b19:	c5 fa 59 f8          	vmulss %xmm0,%xmm0,%xmm7
     b1d:	c5 ca 58 f7          	vaddss %xmm7,%xmm6,%xmm6
     b21:	c5 f0 59 c9          	vmulps %xmm1,%xmm1,%xmm1
     b25:	c5 f1 c6 f9 01       	vshufpd $0x1,%xmm1,%xmm1,%xmm7
     b2a:	c5 f0 c6 c9 5f       	vshufps $0x5f,%xmm1,%xmm1,%xmm1
     b2f:	c5 f2 58 cf          	vaddss %xmm7,%xmm1,%xmm1
     b33:	c4 c1 1a 59 fc       	vmulss %xmm12,%xmm12,%xmm7
     b38:	c5 f2 58 cf          	vaddss %xmm7,%xmm1,%xmm1
     b3c:	c5 f8 28 44 24 d0    	vmovaps -0x30(%rsp),%xmm0
     b42:	c5 f8 59 f8          	vmulps %xmm0,%xmm0,%xmm7
     b46:	c5 7a 16 cf          	vmovshdup %xmm7,%xmm9
     b4a:	c5 b2 58 ff          	vaddss %xmm7,%xmm9,%xmm7
     b4e:	c5 f2 59 0d 00 00 00 	vmulss 0x0(%rip),%xmm1,%xmm1        # b56 <simulate+0xb56>
     b55:	00 
     b56:	c5 fa 10 44 24 f0    	vmovss -0x10(%rsp),%xmm0
     b5c:	c5 fa 59 e0          	vmulss %xmm0,%xmm0,%xmm4
     b60:	c5 c2 58 e4          	vaddss %xmm4,%xmm7,%xmm4
     b64:	c5 da 59 25 00 00 00 	vmulss 0x0(%rip),%xmm4,%xmm4        # b6c <simulate+0xb6c>
     b6b:	00 
     b6c:	c5 f2 58 cc          	vaddss %xmm4,%xmm1,%xmm1
     b70:	c5 f8 28 84 24 80 00 	vmovaps 0x80(%rsp),%xmm0
     b77:	00 00 
     b79:	c5 fa 59 e0          	vmulss %xmm0,%xmm0,%xmm4
     b7d:	c5 ea 59 d2          	vmulss %xmm2,%xmm2,%xmm2
     b81:	c5 ea 58 d4          	vaddss %xmm4,%xmm2,%xmm2
     b85:	c5 f8 28 44 24 50    	vmovaps 0x50(%rsp),%xmm0
     b8b:	c5 fa 59 e0          	vmulss %xmm0,%xmm0,%xmm4
     b8f:	c5 ea 58 d4          	vaddss %xmm4,%xmm2,%xmm2
     b93:	c5 ea 59 15 00 00 00 	vmulss 0x0(%rip),%xmm2,%xmm2        # b9b <simulate+0xb9b>
     b9a:	00 
     b9b:	c5 f2 58 ca          	vaddss %xmm2,%xmm1,%xmm1
     b9f:	c5 f8 28 04 24       	vmovaps (%rsp),%xmm0
     ba4:	c5 f8 59 d0          	vmulps %xmm0,%xmm0,%xmm2
     ba8:	c5 e9 c6 e2 01       	vshufpd $0x1,%xmm2,%xmm2,%xmm4
     bad:	c5 e8 c6 d2 5f       	vshufps $0x5f,%xmm2,%xmm2,%xmm2
     bb2:	c5 ea 58 d4          	vaddss %xmm4,%xmm2,%xmm2
     bb6:	c5 fa 10 44 24 8c    	vmovss -0x74(%rsp),%xmm0
     bbc:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
     bc0:	c5 ea 58 c0          	vaddss %xmm0,%xmm2,%xmm0
     bc4:	c5 ca 59 15 00 00 00 	vmulss 0x0(%rip),%xmm6,%xmm2        # bcc <simulate+0xbcc>
     bcb:	00 
     bcc:	c5 fa 59 05 00 00 00 	vmulss 0x0(%rip),%xmm0,%xmm0        # bd4 <simulate+0xbd4>
     bd3:	00 
     bd4:	c5 f2 58 c0          	vaddss %xmm0,%xmm1,%xmm0
     bd8:	c5 ea 58 c0          	vaddss %xmm0,%xmm2,%xmm0
     bdc:	c5 fa 11 04 24       	vmovss %xmm0,(%rsp)
     be1:	c5 f9 28 5c 24 e0    	vmovapd -0x20(%rsp),%xmm3
     be7:	c5 e1 c6 c3 01       	vshufpd $0x1,%xmm3,%xmm3,%xmm0
     bec:	c5 f9 29 44 24 d0    	vmovapd %xmm0,-0x30(%rsp)
     bf2:	c4 41 78 28 d0       	vmovaps %xmm8,%xmm10
     bf7:	c5 ba 5c c8          	vsubss %xmm0,%xmm8,%xmm1
     bfb:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
     bff:	c5 7a 10 74 24 20    	vmovss 0x20(%rsp),%xmm14
     c05:	c5 8a 5c 54 24 b0    	vsubss -0x50(%rsp),%xmm14,%xmm2
     c0b:	c5 ea 59 d2          	vmulss %xmm2,%xmm2,%xmm2
     c0f:	c5 f2 58 ca          	vaddss %xmm2,%xmm1,%xmm1
     c13:	c5 e0 c6 e3 ff       	vshufps $0xff,%xmm3,%xmm3,%xmm4
     c18:	c4 41 78 28 cb       	vmovaps %xmm11,%xmm9
     c1d:	c4 c1 7a 16 d8       	vmovshdup %xmm8,%xmm3
     c22:	c5 e2 5c d4          	vsubss %xmm4,%xmm3,%xmm2
     c26:	c5 ea 59 d2          	vmulss %xmm2,%xmm2,%xmm2
     c2a:	c5 ea 58 c9          	vaddss %xmm1,%xmm2,%xmm1
     c2e:	c5 7a 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm13        # c36 <simulate+0xc36>
     c35:	00 
     c36:	c5 92 59 d1          	vmulss %xmm1,%xmm13,%xmm2
     c3a:	c5 78 29 ee          	vmovaps %xmm13,%xmm6
     c3e:	c5 7a 10 3d 00 00 00 	vmovss 0x0(%rip),%xmm15        # c46 <simulate+0xc46>
     c45:	00 
     c46:	c5 82 58 d2          	vaddss %xmm2,%xmm15,%xmm2
     c4a:	c5 f2 5e ea          	vdivss %xmm2,%xmm1,%xmm5
     c4e:	c5 d2 58 d2          	vaddss %xmm2,%xmm5,%xmm2
     c52:	c5 7a 10 25 00 00 00 	vmovss 0x0(%rip),%xmm12        # c5a <simulate+0xc5a>
     c59:	00 
     c5a:	c5 9a 59 d2          	vmulss %xmm2,%xmm12,%xmm2
     c5e:	c5 f2 5e ea          	vdivss %xmm2,%xmm1,%xmm5
     c62:	c5 d2 58 d2          	vaddss %xmm2,%xmm5,%xmm2
     c66:	c5 9a 59 d2          	vmulss %xmm2,%xmm12,%xmm2
     c6a:	c5 f2 5e ea          	vdivss %xmm2,%xmm1,%xmm5
     c6e:	c5 d2 58 d2          	vaddss %xmm2,%xmm5,%xmm2
     c72:	c5 9a 59 d2          	vmulss %xmm2,%xmm12,%xmm2
     c76:	c5 f2 5e ca          	vdivss %xmm2,%xmm1,%xmm1
     c7a:	c5 f2 58 ca          	vaddss %xmm2,%xmm1,%xmm1
     c7e:	c5 fa 11 4c 24 8c    	vmovss %xmm1,-0x74(%rsp)
     c84:	c5 78 28 ac 24 a0 00 	vmovaps 0xa0(%rsp),%xmm13
     c8b:	00 00 
     c8d:	c4 c1 3a 5c cd       	vsubss %xmm13,%xmm8,%xmm1
     c92:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
     c96:	c5 8a 5c 54 24 30    	vsubss 0x30(%rsp),%xmm14,%xmm2
     c9c:	c5 ea 59 d2          	vmulss %xmm2,%xmm2,%xmm2
     ca0:	c5 f2 58 ca          	vaddss %xmm2,%xmm1,%xmm1
     ca4:	c4 c1 7a 16 c5       	vmovshdup %xmm13,%xmm0
     ca9:	c5 e2 5c d0          	vsubss %xmm0,%xmm3,%xmm2
     cad:	c5 f8 29 84 24 80 00 	vmovaps %xmm0,0x80(%rsp)
     cb4:	00 00 
     cb6:	c5 ea 59 d2          	vmulss %xmm2,%xmm2,%xmm2
     cba:	c5 ea 58 c9          	vaddss %xmm1,%xmm2,%xmm1
     cbe:	c5 f2 59 d6          	vmulss %xmm6,%xmm1,%xmm2
     cc2:	c5 78 28 c6          	vmovaps %xmm6,%xmm8
     cc6:	c5 82 58 d2          	vaddss %xmm2,%xmm15,%xmm2
     cca:	c5 78 29 fe          	vmovaps %xmm15,%xmm6
     cce:	c5 f2 5e ea          	vdivss %xmm2,%xmm1,%xmm5
     cd2:	c5 d2 58 d2          	vaddss %xmm2,%xmm5,%xmm2
     cd6:	c5 9a 59 d2          	vmulss %xmm2,%xmm12,%xmm2
     cda:	c5 f2 5e ea          	vdivss %xmm2,%xmm1,%xmm5
     cde:	c5 d2 58 d2          	vaddss %xmm2,%xmm5,%xmm2
     ce2:	c5 9a 59 d2          	vmulss %xmm2,%xmm12,%xmm2
     ce6:	c5 f2 5e ea          	vdivss %xmm2,%xmm1,%xmm5
     cea:	c5 d2 58 d2          	vaddss %xmm2,%xmm5,%xmm2
     cee:	c5 9a 59 d2          	vmulss %xmm2,%xmm12,%xmm2
     cf2:	c5 f2 5e ca          	vdivss %xmm2,%xmm1,%xmm1
     cf6:	c5 f2 58 ca          	vaddss %xmm2,%xmm1,%xmm1
     cfa:	c5 fa 11 4c 24 f0    	vmovss %xmm1,-0x10(%rsp)
     d00:	c5 78 28 9c 24 90 00 	vmovaps 0x90(%rsp),%xmm11
     d07:	00 00 
     d09:	c4 c1 2a 5c cb       	vsubss %xmm11,%xmm10,%xmm1
     d0e:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
     d12:	c5 8a 5c 54 24 40    	vsubss 0x40(%rsp),%xmm14,%xmm2
     d18:	c5 ea 59 d2          	vmulss %xmm2,%xmm2,%xmm2
     d1c:	c5 f2 58 d2          	vaddss %xmm2,%xmm1,%xmm2
     d20:	c4 c1 7a 16 cb       	vmovshdup %xmm11,%xmm1
     d25:	c5 f8 29 4c 24 a0    	vmovaps %xmm1,-0x60(%rsp)
     d2b:	c5 e2 5c e9          	vsubss %xmm1,%xmm3,%xmm5
     d2f:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     d33:	c5 d2 58 d2          	vaddss %xmm2,%xmm5,%xmm2
     d37:	c5 ba 59 ea          	vmulss %xmm2,%xmm8,%xmm5
     d3b:	c4 41 78 28 f8       	vmovaps %xmm8,%xmm15
     d40:	c5 f8 28 ce          	vmovaps %xmm6,%xmm1
     d44:	c5 d2 58 ee          	vaddss %xmm6,%xmm5,%xmm5
     d48:	c5 ea 5e f5          	vdivss %xmm5,%xmm2,%xmm6
     d4c:	c5 ca 58 ed          	vaddss %xmm5,%xmm6,%xmm5
     d50:	c5 9a 59 ed          	vmulss %xmm5,%xmm12,%xmm5
     d54:	c5 ea 5e f5          	vdivss %xmm5,%xmm2,%xmm6
     d58:	c5 ca 58 ed          	vaddss %xmm5,%xmm6,%xmm5
     d5c:	c5 9a 59 ed          	vmulss %xmm5,%xmm12,%xmm5
     d60:	c5 ea 5e f5          	vdivss %xmm5,%xmm2,%xmm6
     d64:	c5 ca 58 ed          	vaddss %xmm5,%xmm6,%xmm5
     d68:	c5 9a 59 ed          	vmulss %xmm5,%xmm12,%xmm5
     d6c:	c5 ea 5e d5          	vdivss %xmm5,%xmm2,%xmm2
     d70:	c5 ea 58 d5          	vaddss %xmm5,%xmm2,%xmm2
     d74:	c5 fa 11 54 24 e0    	vmovss %xmm2,-0x20(%rsp)
     d7a:	c5 f9 28 74 24 10    	vmovapd 0x10(%rsp),%xmm6
     d80:	c5 c9 c6 d6 01       	vshufpd $0x1,%xmm6,%xmm6,%xmm2
     d85:	c5 f9 29 54 24 60    	vmovapd %xmm2,0x60(%rsp)
     d8b:	c5 aa 5c d2          	vsubss %xmm2,%xmm10,%xmm2
     d8f:	c5 ea 59 d2          	vmulss %xmm2,%xmm2,%xmm2
     d93:	c4 c1 0a 5c e9       	vsubss %xmm9,%xmm14,%xmm5
     d98:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     d9c:	c5 ea 58 d5          	vaddss %xmm5,%xmm2,%xmm2
     da0:	c5 48 c6 d6 ff       	vshufps $0xff,%xmm6,%xmm6,%xmm10
     da5:	c4 c1 62 5c da       	vsubss %xmm10,%xmm3,%xmm3
     daa:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     dae:	c5 e2 58 d2          	vaddss %xmm2,%xmm3,%xmm2
     db2:	c5 ba 59 da          	vmulss %xmm2,%xmm8,%xmm3
     db6:	c5 e2 58 d9          	vaddss %xmm1,%xmm3,%xmm3
     dba:	c5 f8 28 f9          	vmovaps %xmm1,%xmm7
     dbe:	c5 ea 5e eb          	vdivss %xmm3,%xmm2,%xmm5
     dc2:	c5 d2 58 db          	vaddss %xmm3,%xmm5,%xmm3
     dc6:	c5 9a 59 db          	vmulss %xmm3,%xmm12,%xmm3
     dca:	c5 ea 5e eb          	vdivss %xmm3,%xmm2,%xmm5
     dce:	c5 d2 58 db          	vaddss %xmm3,%xmm5,%xmm3
     dd2:	c5 9a 59 db          	vmulss %xmm3,%xmm12,%xmm3
     dd6:	c5 ea 5e eb          	vdivss %xmm3,%xmm2,%xmm5
     dda:	c5 d2 58 db          	vaddss %xmm3,%xmm5,%xmm3
     dde:	c5 9a 59 db          	vmulss %xmm3,%xmm12,%xmm3
     de2:	c5 ea 5e d3          	vdivss %xmm3,%xmm2,%xmm2
     de6:	c5 ea 58 d3          	vaddss %xmm3,%xmm2,%xmm2
     dea:	c5 fa 11 54 24 20    	vmovss %xmm2,0x20(%rsp)
     df0:	c5 f8 28 4c 24 d0    	vmovaps -0x30(%rsp),%xmm1
     df6:	c4 c1 72 5c dd       	vsubss %xmm13,%xmm1,%xmm3
     dfb:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     dff:	c5 78 28 f4          	vmovaps %xmm4,%xmm14
     e03:	c5 da 5c e8          	vsubss %xmm0,%xmm4,%xmm5
     e07:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     e0b:	c5 d2 58 db          	vaddss %xmm3,%xmm5,%xmm3
     e0f:	c5 7a 10 44 24 30    	vmovss 0x30(%rsp),%xmm8
     e15:	c5 fa 10 64 24 b0    	vmovss -0x50(%rsp),%xmm4
     e1b:	c4 c1 5a 5c e8       	vsubss %xmm8,%xmm4,%xmm5
     e20:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     e24:	c5 e2 58 dd          	vaddss %xmm5,%xmm3,%xmm3
     e28:	c5 82 59 eb          	vmulss %xmm3,%xmm15,%xmm5
     e2c:	c5 d2 58 ef          	vaddss %xmm7,%xmm5,%xmm5
     e30:	c5 e2 5e f5          	vdivss %xmm5,%xmm3,%xmm6
     e34:	c5 ca 58 ed          	vaddss %xmm5,%xmm6,%xmm5
     e38:	c5 9a 59 ed          	vmulss %xmm5,%xmm12,%xmm5
     e3c:	c5 e2 5e f5          	vdivss %xmm5,%xmm3,%xmm6
     e40:	c5 ca 58 ed          	vaddss %xmm5,%xmm6,%xmm5
     e44:	c5 9a 59 ed          	vmulss %xmm5,%xmm12,%xmm5
     e48:	c5 e2 5e f5          	vdivss %xmm5,%xmm3,%xmm6
     e4c:	c5 ca 58 ed          	vaddss %xmm5,%xmm6,%xmm5
     e50:	c5 9a 59 ed          	vmulss %xmm5,%xmm12,%xmm5
     e54:	c5 e2 5e dd          	vdivss %xmm5,%xmm3,%xmm3
     e58:	c5 e2 58 c5          	vaddss %xmm5,%xmm3,%xmm0
     e5c:	c5 fa 11 44 24 10    	vmovss %xmm0,0x10(%rsp)
     e62:	c4 c1 72 5c db       	vsubss %xmm11,%xmm1,%xmm3
     e67:	c5 f8 28 c1          	vmovaps %xmm1,%xmm0
     e6b:	c5 e2 59 db          	vmulss %xmm3,%xmm3,%xmm3
     e6f:	c5 fa 10 4c 24 40    	vmovss 0x40(%rsp),%xmm1
     e75:	c5 da 5c e9          	vsubss %xmm1,%xmm4,%xmm5
     e79:	c5 f8 28 fc          	vmovaps %xmm4,%xmm7
     e7d:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     e81:	c5 e2 58 dd          	vaddss %xmm5,%xmm3,%xmm3
     e85:	c5 f8 28 54 24 a0    	vmovaps -0x60(%rsp),%xmm2
     e8b:	c5 8a 5c ea          	vsubss %xmm2,%xmm14,%xmm5
     e8f:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     e93:	c5 d2 58 db          	vaddss %xmm3,%xmm5,%xmm3
     e97:	c5 82 59 eb          	vmulss %xmm3,%xmm15,%xmm5
     e9b:	c5 d2 58 2d 00 00 00 	vaddss 0x0(%rip),%xmm5,%xmm5        # ea3 <simulate+0xea3>
     ea2:	00 
     ea3:	c5 e2 5e f5          	vdivss %xmm5,%xmm3,%xmm6
     ea7:	c5 ca 58 ed          	vaddss %xmm5,%xmm6,%xmm5
     eab:	c5 9a 59 ed          	vmulss %xmm5,%xmm12,%xmm5
     eaf:	c5 e2 5e f5          	vdivss %xmm5,%xmm3,%xmm6
     eb3:	c5 ca 58 ed          	vaddss %xmm5,%xmm6,%xmm5
     eb7:	c5 9a 59 ed          	vmulss %xmm5,%xmm12,%xmm5
     ebb:	c5 e2 5e f5          	vdivss %xmm5,%xmm3,%xmm6
     ebf:	c5 ca 58 ed          	vaddss %xmm5,%xmm6,%xmm5
     ec3:	c5 9a 59 ed          	vmulss %xmm5,%xmm12,%xmm5
     ec7:	c5 e2 5e dd          	vdivss %xmm5,%xmm3,%xmm3
     ecb:	c5 e2 58 dd          	vaddss %xmm5,%xmm3,%xmm3
     ecf:	c5 fa 11 5c 24 90    	vmovss %xmm3,-0x70(%rsp)
     ed5:	c5 78 28 7c 24 60    	vmovaps 0x60(%rsp),%xmm15
     edb:	c4 c1 7a 5c c7       	vsubss %xmm15,%xmm0,%xmm0
     ee0:	c5 fa 59 c0          	vmulss %xmm0,%xmm0,%xmm0
     ee4:	c4 c1 0a 5c e2       	vsubss %xmm10,%xmm14,%xmm4
     ee9:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
     eed:	c5 da 58 c0          	vaddss %xmm0,%xmm4,%xmm0
     ef1:	c4 41 78 28 f1       	vmovaps %xmm9,%xmm14
     ef6:	c4 c1 42 5c e1       	vsubss %xmm9,%xmm7,%xmm4
     efb:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
     eff:	c5 fa 58 c4          	vaddss %xmm4,%xmm0,%xmm0
     f03:	c5 fa 10 1d 00 00 00 	vmovss 0x0(%rip),%xmm3        # f0b <simulate+0xf0b>
     f0a:	00 
     f0b:	c5 fa 59 e3          	vmulss %xmm3,%xmm0,%xmm4
     f0f:	c5 7a 10 0d 00 00 00 	vmovss 0x0(%rip),%xmm9        # f17 <simulate+0xf17>
     f16:	00 
     f17:	c5 b2 58 e4          	vaddss %xmm4,%xmm9,%xmm4
     f1b:	c5 fa 5e ec          	vdivss %xmm4,%xmm0,%xmm5
     f1f:	c5 d2 58 e4          	vaddss %xmm4,%xmm5,%xmm4
     f23:	c5 9a 59 e4          	vmulss %xmm4,%xmm12,%xmm4
     f27:	c5 fa 5e ec          	vdivss %xmm4,%xmm0,%xmm5
     f2b:	c5 d2 58 e4          	vaddss %xmm4,%xmm5,%xmm4
     f2f:	c5 9a 59 e4          	vmulss %xmm4,%xmm12,%xmm4
     f33:	c5 fa 5e ec          	vdivss %xmm4,%xmm0,%xmm5
     f37:	c5 d2 58 e4          	vaddss %xmm4,%xmm5,%xmm4
     f3b:	c5 9a 59 e4          	vmulss %xmm4,%xmm12,%xmm4
     f3f:	c5 fa 5e c4          	vdivss %xmm4,%xmm0,%xmm0
     f43:	c5 fa 58 c4          	vaddss %xmm4,%xmm0,%xmm0
     f47:	c5 fa 11 44 24 b0    	vmovss %xmm0,-0x50(%rsp)
     f4d:	c4 c1 12 5c e3       	vsubss %xmm11,%xmm13,%xmm4
     f52:	c5 da 59 e4          	vmulss %xmm4,%xmm4,%xmm4
     f56:	c5 78 29 c0          	vmovaps %xmm8,%xmm0
     f5a:	c5 ba 5c e9          	vsubss %xmm1,%xmm8,%xmm5
     f5e:	c5 78 28 c1          	vmovaps %xmm1,%xmm8
     f62:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     f66:	c5 da 58 e5          	vaddss %xmm5,%xmm4,%xmm4
     f6a:	c5 f8 28 8c 24 80 00 	vmovaps 0x80(%rsp),%xmm1
     f71:	00 00 
     f73:	c5 f2 5c ea          	vsubss %xmm2,%xmm1,%xmm5
     f77:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     f7b:	c5 d2 58 e4          	vaddss %xmm4,%xmm5,%xmm4
     f7f:	c5 da 59 eb          	vmulss %xmm3,%xmm4,%xmm5
     f83:	c5 b2 58 ed          	vaddss %xmm5,%xmm9,%xmm5
     f87:	c5 da 5e f5          	vdivss %xmm5,%xmm4,%xmm6
     f8b:	c5 ca 58 ed          	vaddss %xmm5,%xmm6,%xmm5
     f8f:	c5 9a 59 ed          	vmulss %xmm5,%xmm12,%xmm5
     f93:	c5 da 5e f5          	vdivss %xmm5,%xmm4,%xmm6
     f97:	c5 ca 58 ed          	vaddss %xmm5,%xmm6,%xmm5
     f9b:	c5 9a 59 ed          	vmulss %xmm5,%xmm12,%xmm5
     f9f:	c5 da 5e f5          	vdivss %xmm5,%xmm4,%xmm6
     fa3:	c5 ca 58 ed          	vaddss %xmm5,%xmm6,%xmm5
     fa7:	c5 9a 59 ed          	vmulss %xmm5,%xmm12,%xmm5
     fab:	c5 da 5e e5          	vdivss %xmm5,%xmm4,%xmm4
     faf:	c5 da 58 e5          	vaddss %xmm5,%xmm4,%xmm4
     fb3:	c4 c1 12 5c ef       	vsubss %xmm15,%xmm13,%xmm5
     fb8:	c5 d2 59 ed          	vmulss %xmm5,%xmm5,%xmm5
     fbc:	c4 c1 72 5c f2       	vsubss %xmm10,%xmm1,%xmm6
     fc1:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
     fc5:	c5 ca 58 ed          	vaddss %xmm5,%xmm6,%xmm5
     fc9:	c4 c1 7a 5c f6       	vsubss %xmm14,%xmm0,%xmm6
     fce:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
     fd2:	c5 d2 58 ee          	vaddss %xmm6,%xmm5,%xmm5
     fd6:	c5 d2 59 f3          	vmulss %xmm3,%xmm5,%xmm6
     fda:	c5 b2 58 f6          	vaddss %xmm6,%xmm9,%xmm6
     fde:	c5 d2 5e fe          	vdivss %xmm6,%xmm5,%xmm7
     fe2:	c5 c2 58 f6          	vaddss %xmm6,%xmm7,%xmm6
     fe6:	c5 9a 59 f6          	vmulss %xmm6,%xmm12,%xmm6
     fea:	c5 d2 5e fe          	vdivss %xmm6,%xmm5,%xmm7
     fee:	c5 c2 58 f6          	vaddss %xmm6,%xmm7,%xmm6
     ff2:	c5 9a 59 f6          	vmulss %xmm6,%xmm12,%xmm6
     ff6:	c5 d2 5e fe          	vdivss %xmm6,%xmm5,%xmm7
     ffa:	c5 c2 58 f6          	vaddss %xmm6,%xmm7,%xmm6
     ffe:	c5 9a 59 f6          	vmulss %xmm6,%xmm12,%xmm6
    1002:	c5 d2 5e ee          	vdivss %xmm6,%xmm5,%xmm5
    1006:	c5 d2 58 ee          	vaddss %xmm6,%xmm5,%xmm5
    100a:	c4 c1 22 5c f7       	vsubss %xmm15,%xmm11,%xmm6
    100f:	c4 c1 6a 5c ca       	vsubss %xmm10,%xmm2,%xmm1
    1014:	c4 c1 3a 5c fe       	vsubss %xmm14,%xmm8,%xmm7
    1019:	c5 ca 59 f6          	vmulss %xmm6,%xmm6,%xmm6
    101d:	c5 c2 59 ff          	vmulss %xmm7,%xmm7,%xmm7
    1021:	c5 ca 58 f7          	vaddss %xmm7,%xmm6,%xmm6
    1025:	c5 f2 59 c9          	vmulss %xmm1,%xmm1,%xmm1
    1029:	c5 f2 58 ce          	vaddss %xmm6,%xmm1,%xmm1
    102d:	c5 f2 59 f3          	vmulss %xmm3,%xmm1,%xmm6
    1031:	c5 b2 58 f6          	vaddss %xmm6,%xmm9,%xmm6
    1035:	c5 f2 5e fe          	vdivss %xmm6,%xmm1,%xmm7
    1039:	c5 c2 58 f6          	vaddss %xmm6,%xmm7,%xmm6
    103d:	c5 9a 59 f6          	vmulss %xmm6,%xmm12,%xmm6
    1041:	c5 f2 5e fe          	vdivss %xmm6,%xmm1,%xmm7
    1045:	c5 c2 58 f6          	vaddss %xmm6,%xmm7,%xmm6
    1049:	c5 9a 59 f6          	vmulss %xmm6,%xmm12,%xmm6
    104d:	c5 f2 5e fe          	vdivss %xmm6,%xmm1,%xmm7
    1051:	c5 c2 58 f6          	vaddss %xmm6,%xmm7,%xmm6
    1055:	c5 9a 59 f6          	vmulss %xmm6,%xmm12,%xmm6
    1059:	c5 f2 5e ce          	vdivss %xmm6,%xmm1,%xmm1
    105d:	c5 f2 58 ce          	vaddss %xmm6,%xmm1,%xmm1
    1061:	c5 fa 10 35 00 00 00 	vmovss 0x0(%rip),%xmm6        # 1069 <simulate+0x1069>
    1068:	00 
    1069:	c5 ca 5e c9          	vdivss %xmm1,%xmm6,%xmm1
    106d:	c5 fa 10 35 00 00 00 	vmovss 0x0(%rip),%xmm6        # 1075 <simulate+0x1075>
    1074:	00 
    1075:	c5 ca 5e ed          	vdivss %xmm5,%xmm6,%xmm5
    1079:	c5 d2 58 c9          	vaddss %xmm1,%xmm5,%xmm1
    107d:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # 1085 <simulate+0x1085>
    1084:	00 
    1085:	c5 d2 5e e4          	vdivss %xmm4,%xmm5,%xmm4
    1089:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # 1091 <simulate+0x1091>
    1090:	00 
    1091:	c5 d2 5e 44 24 b0    	vdivss -0x50(%rsp),%xmm5,%xmm0
    1097:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # 109f <simulate+0x109f>
    109e:	00 
    109f:	c5 d2 5e 5c 24 90    	vdivss -0x70(%rsp),%xmm5,%xmm3
    10a5:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # 10ad <simulate+0x10ad>
    10ac:	00 
    10ad:	c5 d2 5e 6c 24 10    	vdivss 0x10(%rsp),%xmm5,%xmm5
    10b3:	c5 d2 58 db          	vaddss %xmm3,%xmm5,%xmm3
    10b7:	c5 e2 58 dc          	vaddss %xmm4,%xmm3,%xmm3
    10bb:	c5 fa 10 25 00 00 00 	vmovss 0x0(%rip),%xmm4        # 10c3 <simulate+0x10c3>
    10c2:	00 
    10c3:	c5 da 5e 64 24 e0    	vdivss -0x20(%rsp),%xmm4,%xmm4
    10c9:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # 10d1 <simulate+0x10d1>
    10d0:	00 
    10d1:	c5 d2 5e 6c 24 8c    	vdivss -0x74(%rsp),%xmm5,%xmm5
    10d7:	c5 d2 58 e4          	vaddss %xmm4,%xmm5,%xmm4
    10db:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # 10e3 <simulate+0x10e3>
    10e2:	00 
    10e3:	c5 d2 5e 6c 24 f0    	vdivss -0x10(%rsp),%xmm5,%xmm5
    10e9:	c5 d2 58 2c 24       	vaddss (%rsp),%xmm5,%xmm5
    10ee:	c5 d2 58 e4          	vaddss %xmm4,%xmm5,%xmm4
    10f2:	c5 fa 10 2d 00 00 00 	vmovss 0x0(%rip),%xmm5        # 10fa <simulate+0x10fa>
    10f9:	00 
    10fa:	c5 d2 5e 54 24 20    	vdivss 0x20(%rsp),%xmm5,%xmm2
    1100:	c5 e2 58 d2          	vaddss %xmm2,%xmm3,%xmm2
    1104:	c5 ea 58 c0          	vaddss %xmm0,%xmm2,%xmm0
    1108:	c5 da 58 c0          	vaddss %xmm0,%xmm4,%xmm0
    110c:	c5 fa 58 c1          	vaddss %xmm1,%xmm0,%xmm0
    1110:	48 81 c4 e8 00 00 00 	add    $0xe8,%rsp
    1117:	e9 00 00 00 00       	jmp    111c <simulate+0x111c>
    111c:	48 81 c4 e8 00 00 00 	add    $0xe8,%rsp
    1123:	c3                   	ret
    1124:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
    112b:	00 00 00 00 00 

0000000000001130 <init_state>:
    1130:	53                   	push   %rbx
    1131:	48 89 fb             	mov    %rdi,%rbx
    1134:	bf 00 00 00 00       	mov    $0x0,%edi
    1139:	e8 00 00 00 00       	call   113e <init_state+0xe>
    113e:	48 89 03             	mov    %rax,(%rbx)
    1141:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    1145:	c5 f8 11 43 18       	vmovups %xmm0,0x18(%rbx)
    114a:	c5 f8 11 43 08       	vmovups %xmm0,0x8(%rbx)
    114f:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # 1157 <init_state+0x27>
    1156:	00 
    1157:	c5 f8 11 43 28       	vmovups %xmm0,0x28(%rbx)
    115c:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # 1164 <init_state+0x34>
    1163:	00 
    1164:	c5 f8 11 43 38       	vmovups %xmm0,0x38(%rbx)
    1169:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # 1171 <init_state+0x41>
    1170:	00 
    1171:	c5 f8 11 43 48       	vmovups %xmm0,0x48(%rbx)
    1176:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # 117e <init_state+0x4e>
    117d:	00 
    117e:	c5 f8 11 43 58       	vmovups %xmm0,0x58(%rbx)
    1183:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # 118b <init_state+0x5b>
    118a:	00 
    118b:	c5 f8 11 43 68       	vmovups %xmm0,0x68(%rbx)
    1190:	c5 f8 28 05 00 00 00 	vmovaps 0x0(%rip),%xmm0        # 1198 <init_state+0x68>
    1197:	00 
    1198:	c5 f8 11 43 78       	vmovups %xmm0,0x78(%rbx)
    119d:	5b                   	pop    %rbx
    119e:	c3                   	ret
    119f:	90                   	nop

00000000000011a0 <main>:
    11a0:	50                   	push   %rax
    11a1:	bf 00 00 00 00       	mov    $0x0,%edi
    11a6:	e8 00 00 00 00       	call   11ab <main+0xb>
    11ab:	31 c9                	xor    %ecx,%ecx
    11ad:	0f 1f 00             	nopl   (%rax)
    11b0:	31 d2                	xor    %edx,%edx
    11b2:	48 39 c1             	cmp    %rax,%rcx
    11b5:	0f 9c c2             	setl   %dl
    11b8:	48 01 d1             	add    %rdx,%rcx
    11bb:	48 39 c1             	cmp    %rax,%rcx
    11be:	75 f0                	jne    11b0 <main+0x10>
    11c0:	31 c0                	xor    %eax,%eax
    11c2:	59                   	pop    %rcx
    11c3:	c3                   	ret
    11c4:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
    11cb:	00 00 00 00 00 

00000000000011d0 <briev_rt_ctor>:
    11d0:	e9 00 00 00 00       	jmp    11d5 <briev_rt_ctor+0x5>
    11d5:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
    11dc:	00 00 00 00 

00000000000011e0 <__rt_init>:
    11e0:	53                   	push   %rbx
    11e1:	48 81 ec 00 01 00 00 	sub    $0x100,%rsp
    11e8:	be 00 00 00 00       	mov    $0x0,%esi
    11ed:	bf 02 00 00 00       	mov    $0x2,%edi
    11f2:	e8 00 00 00 00       	call   11f7 <__rt_init+0x17>
    11f7:	be 00 00 00 00       	mov    $0x0,%esi
    11fc:	bf 0f 00 00 00       	mov    $0xf,%edi
    1201:	e8 00 00 00 00       	call   1206 <__rt_init+0x26>
    1206:	be 00 00 00 00       	mov    $0x0,%esi
    120b:	bf 01 00 00 00       	mov    $0x1,%edi
    1210:	e8 00 00 00 00       	call   1215 <__rt_init+0x35>
    1215:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    1219:	c5 fc 11 84 24 e0 00 	vmovups %ymm0,0xe0(%rsp)
    1220:	00 00 
    1222:	c5 fc 11 84 24 d0 00 	vmovups %ymm0,0xd0(%rsp)
    1229:	00 00 
    122b:	c5 fc 11 84 24 b0 00 	vmovups %ymm0,0xb0(%rsp)
    1232:	00 00 
    1234:	c5 fc 11 84 24 90 00 	vmovups %ymm0,0x90(%rsp)
    123b:	00 00 
    123d:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
    1243:	48 c7 44 24 68 00 00 	movq   $0x0,0x68(%rsp)
    124a:	00 00 
    124c:	c7 84 24 f0 00 00 00 	movl   $0x4,0xf0(%rsp)
    1253:	04 00 00 00 
    1257:	c5 f8 77             	vzeroupper
    125a:	e8 00 00 00 00       	call   125f <__rt_init+0x7f>
    125f:	8d 78 01             	lea    0x1(%rax),%edi
    1262:	48 8d 5c 24 68       	lea    0x68(%rsp),%rbx
    1267:	48 89 de             	mov    %rbx,%rsi
    126a:	31 d2                	xor    %edx,%edx
    126c:	e8 00 00 00 00       	call   1271 <__rt_init+0x91>
    1271:	e8 00 00 00 00       	call   1276 <__rt_init+0x96>
    1276:	8d 78 02             	lea    0x2(%rax),%edi
    1279:	48 89 de             	mov    %rbx,%rsi
    127c:	31 d2                	xor    %edx,%edx
    127e:	e8 00 00 00 00       	call   1283 <__rt_init+0xa3>
    1283:	e8 00 00 00 00       	call   1288 <__rt_init+0xa8>
    1288:	ff c0                	inc    %eax
    128a:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    128e:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
    1293:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
    1299:	89 44 24 08          	mov    %eax,0x8(%rsp)
    129d:	48 89 e6             	mov    %rsp,%rsi
    12a0:	ba 00 00 00 00       	mov    $0x0,%edx
    12a5:	31 ff                	xor    %edi,%edi
    12a7:	c5 f8 77             	vzeroupper
    12aa:	e8 00 00 00 00       	call   12af <__rt_init+0xcf>
    12af:	85 c0                	test   %eax,%eax
    12b1:	75 27                	jne    12da <__rt_init+0xfa>
    12b3:	c4 e2 7d 1a 05 00 00 	vbroadcastf128 0x0(%rip),%ymm0        # 12bc <__rt_init+0xdc>
    12ba:	00 00 
    12bc:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
    12c2:	48 8b 3d 00 00 00 00 	mov    0x0(%rip),%rdi        # 12c9 <__rt_init+0xe9>
    12c9:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
    12ce:	31 f6                	xor    %esi,%esi
    12d0:	31 c9                	xor    %ecx,%ecx
    12d2:	c5 f8 77             	vzeroupper
    12d5:	e8 00 00 00 00       	call   12da <__rt_init+0xfa>
    12da:	e8 00 00 00 00       	call   12df <__rt_init+0xff>
    12df:	83 c0 02             	add    $0x2,%eax
    12e2:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    12e6:	c5 fc 11 04 24       	vmovups %ymm0,(%rsp)
    12eb:	c5 fc 11 44 24 20    	vmovups %ymm0,0x20(%rsp)
    12f1:	89 44 24 08          	mov    %eax,0x8(%rsp)
    12f5:	48 89 e6             	mov    %rsp,%rsi
    12f8:	ba 00 00 00 00       	mov    $0x0,%edx
    12fd:	31 ff                	xor    %edi,%edi
    12ff:	c5 f8 77             	vzeroupper
    1302:	e8 00 00 00 00       	call   1307 <__rt_init+0x127>
    1307:	85 c0                	test   %eax,%eax
    1309:	75 27                	jne    1332 <__rt_init+0x152>
    130b:	c4 e2 7d 1a 05 00 00 	vbroadcastf128 0x0(%rip),%ymm0        # 1314 <__rt_init+0x134>
    1312:	00 00 
    1314:	c5 fc 11 44 24 48    	vmovups %ymm0,0x48(%rsp)
    131a:	48 8b 3d 00 00 00 00 	mov    0x0(%rip),%rdi        # 1321 <__rt_init+0x141>
    1321:	48 8d 54 24 48       	lea    0x48(%rsp),%rdx
    1326:	31 f6                	xor    %esi,%esi
    1328:	31 c9                	xor    %ecx,%ecx
    132a:	c5 f8 77             	vzeroupper
    132d:	e8 00 00 00 00       	call   1332 <__rt_init+0x152>
    1332:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 1339 <__rt_init+0x159>
    1339:	48 8b 38             	mov    (%rax),%rdi
    133c:	31 f6                	xor    %esi,%esi
    133e:	ba 01 00 00 00       	mov    $0x1,%edx
    1343:	31 c9                	xor    %ecx,%ecx
    1345:	e8 00 00 00 00       	call   134a <__rt_init+0x16a>
    134a:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1351 <__rt_init+0x171>
    1351:	48 81 c4 00 01 00 00 	add    $0x100,%rsp
    1358:	5b                   	pop    %rbx
    1359:	c3                   	ret
    135a:	66 0f 1f 44 00 00    	nopw   0x0(%rax,%rax,1)

0000000000001360 <handle_sigint>:
    1360:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1367 <handle_sigint+0x7>
    1367:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 136e <handle_sigint+0xe>
    136e:	c3                   	ret
    136f:	90                   	nop

0000000000001370 <handle_sigterm>:
    1370:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1377 <handle_sigterm+0x7>
    1377:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 137e <handle_sigterm+0xe>
    137e:	c3                   	ret
    137f:	90                   	nop

0000000000001380 <handle_sighup>:
    1380:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1387 <handle_sighup+0x7>
    1387:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 138e <handle_sighup+0xe>
    138e:	c3                   	ret
    138f:	90                   	nop

0000000000001390 <handle_timer>:
    1390:	48 ff 05 00 00 00 00 	incq   0x0(%rip)        # 1397 <handle_timer+0x7>
    1397:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 139e <handle_timer+0xe>
    139e:	c3                   	ret
    139f:	90                   	nop

00000000000013a0 <__get_env_int>:
    13a0:	53                   	push   %rbx
    13a1:	48 83 ec 10          	sub    $0x10,%rsp
    13a5:	e8 00 00 00 00       	call   13aa <__get_env_int+0xa>
    13aa:	48 85 c0             	test   %rax,%rax
    13ad:	74 32                	je     13e1 <__get_env_int+0x41>
    13af:	48 c7 44 24 08 00 00 	movq   $0x0,0x8(%rsp)
    13b6:	00 00 
    13b8:	48 8d 74 24 08       	lea    0x8(%rsp),%rsi
    13bd:	48 89 c7             	mov    %rax,%rdi
    13c0:	ba 0a 00 00 00       	mov    $0xa,%edx
    13c5:	48 89 c3             	mov    %rax,%rbx
    13c8:	e8 00 00 00 00       	call   13cd <__get_env_int+0x2d>
    13cd:	48 89 c1             	mov    %rax,%rcx
    13d0:	31 c0                	xor    %eax,%eax
    13d2:	48 39 5c 24 08       	cmp    %rbx,0x8(%rsp)
    13d7:	48 0f 45 c1          	cmovne %rcx,%rax
    13db:	48 83 c4 10          	add    $0x10,%rsp
    13df:	5b                   	pop    %rbx
    13e0:	c3                   	ret
    13e1:	31 c0                	xor    %eax,%eax
    13e3:	48 83 c4 10          	add    $0x10,%rsp
    13e7:	5b                   	pop    %rbx
    13e8:	c3                   	ret
    13e9:	0f 1f 80 00 00 00 00 	nopl   0x0(%rax)

00000000000013f0 <__rt_wait>:
    13f0:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
    13f7:	8b 3d 00 00 00 00    	mov    0x0(%rip),%edi        # 13fd <__rt_wait+0xd>
    13fd:	85 ff                	test   %edi,%edi
    13ff:	79 3f                	jns    1440 <__rt_wait+0x50>
    1401:	31 ff                	xor    %edi,%edi
    1403:	e8 00 00 00 00       	call   1408 <__rt_wait+0x18>
    1408:	89 05 00 00 00 00    	mov    %eax,0x0(%rip)        # 140e <__rt_wait+0x1e>
    140e:	85 c0                	test   %eax,%eax
    1410:	0f 88 d5 00 00 00    	js     14eb <__rt_wait+0xfb>
    1416:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
    141d:	00 
    141e:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
    1425:	00 00 
    1427:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
    142c:	89 c7                	mov    %eax,%edi
    142e:	be 01 00 00 00       	mov    $0x1,%esi
    1433:	31 d2                	xor    %edx,%edx
    1435:	e8 00 00 00 00       	call   143a <__rt_wait+0x4a>
    143a:	8b 3d 00 00 00 00    	mov    0x0(%rip),%edi        # 1440 <__rt_wait+0x50>
    1440:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
    1445:	ba 40 00 00 00       	mov    $0x40,%edx
    144a:	b9 64 00 00 00       	mov    $0x64,%ecx
    144f:	e8 00 00 00 00       	call   1454 <__rt_wait+0x64>
    1454:	85 c0                	test   %eax,%eax
    1456:	0f 8e ef 00 00 00    	jle    154b <__rt_wait+0x15b>
    145c:	89 c1                	mov    %eax,%ecx
    145e:	83 f8 01             	cmp    $0x1,%eax
    1461:	75 1e                	jne    1481 <__rt_wait+0x91>
    1463:	31 c0                	xor    %eax,%eax
    1465:	f6 c1 01             	test   $0x1,%cl
    1468:	74 0f                	je     1479 <__rt_wait+0x89>
    146a:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
    146e:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
    1473:	0f 84 e1 00 00 00    	je     155a <__rt_wait+0x16a>
    1479:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
    1480:	c3                   	ret
    1481:	89 c8                	mov    %ecx,%eax
    1483:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
    1488:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
    148d:	48 89 c6             	mov    %rax,%rsi
    1490:	eb 18                	jmp    14aa <__rt_wait+0xba>
    1492:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1499:	1f 84 00 00 00 00 00 
    14a0:	48 83 c2 18          	add    $0x18,%rdx
    14a4:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
    14a8:	74 bb                	je     1465 <__rt_wait+0x75>
    14aa:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
    14ae:	75 20                	jne    14d0 <__rt_wait+0xe0>
    14b0:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
    14b4:	74 1a                	je     14d0 <__rt_wait+0xe0>
    14b6:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 14bd <__rt_wait+0xcd>
    14bd:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 14c4 <__rt_wait+0xd4>
    14c4:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
    14cb:	00 00 00 00 00 
    14d0:	83 3a 00             	cmpl   $0x0,(%rdx)
    14d3:	75 cb                	jne    14a0 <__rt_wait+0xb0>
    14d5:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
    14d9:	74 c5                	je     14a0 <__rt_wait+0xb0>
    14db:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 14e2 <__rt_wait+0xf2>
    14e2:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 14e9 <__rt_wait+0xf9>
    14e9:	eb b5                	jmp    14a0 <__rt_wait+0xb0>
    14eb:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    14ef:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
    14f5:	c5 fc 11 44 24 58    	vmovups %ymm0,0x58(%rsp)
    14fb:	c5 fc 11 44 24 38    	vmovups %ymm0,0x38(%rsp)
    1501:	c5 fc 11 44 24 18    	vmovups %ymm0,0x18(%rsp)
    1507:	48 c7 44 24 10 01 00 	movq   $0x1,0x10(%rsp)
    150e:	00 00 
    1510:	c5 f8 10 05 00 00 00 	vmovups 0x0(%rip),%xmm0        # 1518 <__rt_wait+0x128>
    1517:	00 
    1518:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
    151d:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
    1522:	49 89 e0             	mov    %rsp,%r8
    1525:	bf 01 00 00 00       	mov    $0x1,%edi
    152a:	31 d2                	xor    %edx,%edx
    152c:	31 c9                	xor    %ecx,%ecx
    152e:	c5 f8 77             	vzeroupper
    1531:	e8 00 00 00 00       	call   1536 <__rt_wait+0x146>
    1536:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
    153b:	74 0e                	je     154b <__rt_wait+0x15b>
    153d:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1544 <__rt_wait+0x154>
    1544:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 154b <__rt_wait+0x15b>
    154b:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1552 <__rt_wait+0x162>
    1552:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
    1559:	c3                   	ret
    155a:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
    155f:	0f 84 14 ff ff ff    	je     1479 <__rt_wait+0x89>
    1565:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 156c <__rt_wait+0x17c>
    156c:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1573 <__rt_wait+0x183>
    1573:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
    157a:	c3                   	ret
    157b:	0f 1f 44 00 00       	nopl   0x0(%rax,%rax,1)

0000000000001580 <__rt_poll>:
    1580:	48 81 ec 18 03 00 00 	sub    $0x318,%rsp
    1587:	8b 3d 00 00 00 00    	mov    0x0(%rip),%edi        # 158d <__rt_poll+0xd>
    158d:	85 ff                	test   %edi,%edi
    158f:	79 3f                	jns    15d0 <__rt_poll+0x50>
    1591:	31 ff                	xor    %edi,%edi
    1593:	e8 00 00 00 00       	call   1598 <__rt_poll+0x18>
    1598:	89 05 00 00 00 00    	mov    %eax,0x0(%rip)        # 159e <__rt_poll+0x1e>
    159e:	85 c0                	test   %eax,%eax
    15a0:	0f 88 d5 00 00 00    	js     167b <__rt_poll+0xfb>
    15a6:	c7 44 24 18 00 00 00 	movl   $0x0,0x18(%rsp)
    15ad:	00 
    15ae:	48 c7 44 24 10 01 20 	movq   $0x2001,0x10(%rsp)
    15b5:	00 00 
    15b7:	48 8d 4c 24 10       	lea    0x10(%rsp),%rcx
    15bc:	89 c7                	mov    %eax,%edi
    15be:	be 01 00 00 00       	mov    $0x1,%esi
    15c3:	31 d2                	xor    %edx,%edx
    15c5:	e8 00 00 00 00       	call   15ca <__rt_poll+0x4a>
    15ca:	8b 3d 00 00 00 00    	mov    0x0(%rip),%edi        # 15d0 <__rt_poll+0x50>
    15d0:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
    15d5:	ba 40 00 00 00       	mov    $0x40,%edx
    15da:	31 c9                	xor    %ecx,%ecx
    15dc:	e8 00 00 00 00       	call   15e1 <__rt_poll+0x61>
    15e1:	85 c0                	test   %eax,%eax
    15e3:	7e 1d                	jle    1602 <__rt_poll+0x82>
    15e5:	89 c1                	mov    %eax,%ecx
    15e7:	83 f8 01             	cmp    $0x1,%eax
    15ea:	75 25                	jne    1611 <__rt_poll+0x91>
    15ec:	31 c0                	xor    %eax,%eax
    15ee:	f6 c1 01             	test   $0x1,%cl
    15f1:	74 0f                	je     1602 <__rt_poll+0x82>
    15f3:	48 8d 04 40          	lea    (%rax,%rax,2),%rax
    15f7:	83 7c 84 14 00       	cmpl   $0x0,0x14(%rsp,%rax,4)
    15fc:	0f 84 cd 00 00 00    	je     16cf <__rt_poll+0x14f>
    1602:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1609 <__rt_poll+0x89>
    1609:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
    1610:	c3                   	ret
    1611:	89 c8                	mov    %ecx,%eax
    1613:	25 fe ff ff 7f       	and    $0x7ffffffe,%eax
    1618:	48 8d 54 24 20       	lea    0x20(%rsp),%rdx
    161d:	48 89 c6             	mov    %rax,%rsi
    1620:	eb 18                	jmp    163a <__rt_poll+0xba>
    1622:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1629:	1f 84 00 00 00 00 00 
    1630:	48 83 c2 18          	add    $0x18,%rdx
    1634:	48 83 c6 fe          	add    $0xfffffffffffffffe,%rsi
    1638:	74 b4                	je     15ee <__rt_poll+0x6e>
    163a:	83 7a f4 00          	cmpl   $0x0,-0xc(%rdx)
    163e:	75 20                	jne    1660 <__rt_poll+0xe0>
    1640:	f6 42 f0 01          	testb  $0x1,-0x10(%rdx)
    1644:	74 1a                	je     1660 <__rt_poll+0xe0>
    1646:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 164d <__rt_poll+0xcd>
    164d:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1654 <__rt_poll+0xd4>
    1654:	66 66 66 2e 0f 1f 84 	data16 data16 cs nopw 0x0(%rax,%rax,1)
    165b:	00 00 00 00 00 
    1660:	83 3a 00             	cmpl   $0x0,(%rdx)
    1663:	75 cb                	jne    1630 <__rt_poll+0xb0>
    1665:	f6 42 fc 01          	testb  $0x1,-0x4(%rdx)
    1669:	74 c5                	je     1630 <__rt_poll+0xb0>
    166b:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1672 <__rt_poll+0xf2>
    1672:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 1679 <__rt_poll+0xf9>
    1679:	eb b5                	jmp    1630 <__rt_poll+0xb0>
    167b:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    167f:	c5 fc 11 44 24 70    	vmovups %ymm0,0x70(%rsp)
    1685:	c5 fc 11 44 24 58    	vmovups %ymm0,0x58(%rsp)
    168b:	c5 fc 11 44 24 38    	vmovups %ymm0,0x38(%rsp)
    1691:	c5 fc 11 44 24 18    	vmovups %ymm0,0x18(%rsp)
    1697:	48 c7 44 24 10 01 00 	movq   $0x1,0x10(%rsp)
    169e:	00 00 
    16a0:	c5 f8 57 c0          	vxorps %xmm0,%xmm0,%xmm0
    16a4:	c5 f8 29 04 24       	vmovaps %xmm0,(%rsp)
    16a9:	48 8d 74 24 10       	lea    0x10(%rsp),%rsi
    16ae:	49 89 e0             	mov    %rsp,%r8
    16b1:	bf 01 00 00 00       	mov    $0x1,%edi
    16b6:	31 d2                	xor    %edx,%edx
    16b8:	31 c9                	xor    %ecx,%ecx
    16ba:	c5 f8 77             	vzeroupper
    16bd:	e8 00 00 00 00       	call   16c2 <__rt_poll+0x142>
    16c2:	f6 44 24 10 01       	testb  $0x1,0x10(%rsp)
    16c7:	0f 84 35 ff ff ff    	je     1602 <__rt_poll+0x82>
    16cd:	eb 0b                	jmp    16da <__rt_poll+0x15a>
    16cf:	f6 44 84 10 01       	testb  $0x1,0x10(%rsp,%rax,4)
    16d4:	0f 84 28 ff ff ff    	je     1602 <__rt_poll+0x82>
    16da:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 16e1 <__rt_poll+0x161>
    16e1:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 16e8 <__rt_poll+0x168>
    16e8:	c6 05 00 00 00 00 01 	movb   $0x1,0x0(%rip)        # 16ef <__rt_poll+0x16f>
    16ef:	48 81 c4 18 03 00 00 	add    $0x318,%rsp
    16f6:	c3                   	ret
    16f7:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
    16fe:	00 00 

0000000000001700 <__wait_for_event>:
    1700:	e9 00 00 00 00       	jmp    1705 <__wait_for_event+0x5>
    1705:	66 66 2e 0f 1f 84 00 	data16 cs nopw 0x0(%rax,%rax,1)
    170c:	00 00 00 00 

0000000000001710 <__print>:
    1710:	50                   	push   %rax
    1711:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 1718 <__print+0x8>
    1718:	48 8b 30             	mov    (%rax),%rsi
    171b:	e8 00 00 00 00       	call   1720 <__print+0x10>
    1720:	b8 01 00 00 00       	mov    $0x1,%eax
    1725:	59                   	pop    %rcx
    1726:	c3                   	ret
    1727:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
    172e:	00 00 

0000000000001730 <__print_int>:
    1730:	50                   	push   %rax
    1731:	48 89 fa             	mov    %rdi,%rdx
    1734:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 173b <__print_int+0xb>
    173b:	48 8b 38             	mov    (%rax),%rdi
    173e:	be 00 00 00 00       	mov    $0x0,%esi
    1743:	31 c0                	xor    %eax,%eax
    1745:	e8 00 00 00 00       	call   174a <__print_int+0x1a>
    174a:	b8 01 00 00 00       	mov    $0x1,%eax
    174f:	59                   	pop    %rcx
    1750:	c3                   	ret
    1751:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1758:	0f 1f 84 00 00 00 00 
    175f:	00 

0000000000001760 <__print_float>:
    1760:	50                   	push   %rax
    1761:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 1768 <__print_float+0x8>
    1768:	48 8b 38             	mov    (%rax),%rdi
    176b:	c5 fa 5a c0          	vcvtss2sd %xmm0,%xmm0,%xmm0
    176f:	be 00 00 00 00       	mov    $0x0,%esi
    1774:	b0 01                	mov    $0x1,%al
    1776:	e8 00 00 00 00       	call   177b <__print_float+0x1b>
    177b:	b8 01 00 00 00       	mov    $0x1,%eax
    1780:	59                   	pop    %rcx
    1781:	c3                   	ret
    1782:	66 66 66 66 66 2e 0f 	data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1789:	1f 84 00 00 00 00 00 

0000000000001790 <__sqrtf>:
    1790:	c5 f0 57 c9          	vxorps %xmm1,%xmm1,%xmm1
    1794:	c5 f8 2e c1          	vucomiss %xmm1,%xmm0
    1798:	0f 82 00 00 00 00    	jb     179e <__sqrtf+0xe>
    179e:	c5 fa 51 c0          	vsqrtss %xmm0,%xmm0,%xmm0
    17a2:	c3                   	ret
    17a3:	66 66 66 66 2e 0f 1f 	data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    17aa:	84 00 00 00 00 00 

00000000000017b0 <__exit>:
    17b0:	50                   	push   %rax
    17b1:	31 ff                	xor    %edi,%edi
    17b3:	e8 00 00 00 00       	call   17b8 <__exit+0x8>
    17b8:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
    17bf:	00 

00000000000017c0 <__read_stdin>:
    17c0:	48 89 f2             	mov    %rsi,%rdx
    17c3:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 17ca <__read_stdin+0xa>
    17ca:	48 8b 08             	mov    (%rax),%rcx
    17cd:	be 01 00 00 00       	mov    $0x1,%esi
    17d2:	e9 00 00 00 00       	jmp    17d7 <__read_stdin+0x17>
    17d7:	66 0f 1f 84 00 00 00 	nopw   0x0(%rax,%rax,1)
    17de:	00 00 

00000000000017e0 <__putchar>:
    17e0:	53                   	push   %rbx
    17e1:	48 89 fb             	mov    %rdi,%rbx
    17e4:	48 8b 05 00 00 00 00 	mov    0x0(%rip),%rax        # 17eb <__putchar+0xb>
    17eb:	48 8b 30             	mov    (%rax),%rsi
    17ee:	e8 00 00 00 00       	call   17f3 <__putchar+0x13>
    17f3:	48 89 d8             	mov    %rbx,%rax
    17f6:	5b                   	pop    %rbx
    17f7:	c3                   	ret
    17f8:	0f 1f 84 00 00 00 00 	nopl   0x0(%rax,%rax,1)
    17ff:	00 

0000000000001800 <briev_thread_pool_init>:
    1800:	c3                   	ret
    1801:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1808:	0f 1f 84 00 00 00 00 
    180f:	00 

0000000000001810 <briev_barrier_release>:
    1810:	c3                   	ret
    1811:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1818:	0f 1f 84 00 00 00 00 
    181f:	00 

0000000000001820 <briev_barrier_wait>:
    1820:	c3                   	ret
    1821:	66 66 66 66 66 66 2e 	data16 data16 data16 data16 data16 cs nopw 0x0(%rax,%rax,1)
    1828:	0f 1f 84 00 00 00 00 
    182f:	00 

0000000000001830 <briev_thread_pool_shutdown>:
    1830:	c3                   	ret
