	.text
	.file	"program.bv"
	.globl	get_env                         # -- Begin function get_env
	.p2align	4, 0x90
	.type	get_env,@function
get_env:                                # @get_env
# %bb.0:                                # %entry
	pushq	%rax
	movq	%rsi, %rdi
	callq	__getenv_briv@PLT
	popq	%rcx
	retq
.Lfunc_end0:
	.size	get_env, .Lfunc_end0-get_env
                                        # -- End function
	.globl	get_env_int                     # -- Begin function get_env_int
	.p2align	4, 0x90
	.type	get_env_int,@function
get_env_int:                            # @get_env_int
# %bb.0:                                # %entry
	pushq	%rax
	movq	%rsi, %rdi
	callq	__getenv_int@PLT
	popq	%rcx
	retq
.Lfunc_end1:
	.size	get_env_int, .Lfunc_end1-get_env_int
                                        # -- End function
	.section	.rodata.cst4,"aM",@progbits,4
	.p2align	2, 0x0                          # -- Begin function txn_simulate
.LCPI2_0:
	.long	0xc0400000                      # float -3
.LCPI2_1:
	.long	0xbf000000                      # float -0.5
	.text
	.globl	txn_simulate
	.p2align	4, 0x90
	.type	txn_simulate,@function
txn_simulate:                           # @txn_simulate
# %bb.0:                                # %entry
	movq	8(%rdi), %rax
	cmpq	(%rdi), %rax
	jge	.LBB2_1
# %bb.2:                                # %ps14
	subq	$392, %rsp                      # imm = 0x188
	movss	16(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	20(%rdi), %xmm11                # xmm11 = mem[0],zero,zero,zero
	movaps	%xmm0, %xmm1
	movaps	%xmm0, %xmm6
	subss	%xmm11, %xmm1
	movss	%xmm1, 252(%rsp)                # 4-byte Spill
	movss	36(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	40(%rdi), %xmm10                # xmm10 = mem[0],zero,zero,zero
	movaps	%xmm0, %xmm3
	movaps	%xmm0, %xmm5
	subss	%xmm10, %xmm3
	movss	%xmm3, 256(%rsp)                # 4-byte Spill
	movss	56(%rdi), %xmm9                 # xmm9 = mem[0],zero,zero,zero
	movss	60(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 240(%rsp)                # 4-byte Spill
	movaps	%xmm9, %xmm2
	subss	%xmm0, %xmm2
	movss	%xmm2, 308(%rsp)                # 4-byte Spill
	movaps	%xmm1, %xmm0
	mulss	%xmm1, %xmm0
	movaps	%xmm3, %xmm1
	mulss	%xmm3, %xmm1
	addss	%xmm0, %xmm1
	movaps	%xmm2, %xmm0
	mulss	%xmm2, %xmm0
	addss	%xmm1, %xmm0
	movq	dt@GOTPCREL(%rip), %rax
	movss	(%rax), %xmm3                   # xmm3 = mem[0],zero,zero,zero
	movaps	%xmm0, %xmm2
	mulss	%xmm0, %xmm2
	mulss	%xmm0, %xmm2
	xorps	%xmm0, %xmm0
	rsqrtss	%xmm2, %xmm0
	mulss	%xmm0, %xmm2
	mulss	%xmm0, %xmm2
	movss	.LCPI2_0(%rip), %xmm14          # xmm14 = [-3.0E+0,0.0E+0,0.0E+0,0.0E+0]
	addss	%xmm14, %xmm2
	movaps	%xmm14, %xmm13
	movss	.LCPI2_1(%rip), %xmm14          # xmm14 = [-5.0E-1,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm14, %xmm0
	mulss	%xmm3, %xmm0
	movaps	%xmm3, %xmm8
	mulss	%xmm2, %xmm0
	movss	%xmm0, 376(%rsp)                # 4-byte Spill
	movss	24(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movaps	%xmm6, %xmm1
	subss	%xmm0, %xmm1
	movss	%xmm1, 324(%rsp)                # 4-byte Spill
	movaps	%xmm0, %xmm12
	movss	%xmm0, 224(%rsp)                # 4-byte Spill
	movss	44(%rdi), %xmm7                 # xmm7 = mem[0],zero,zero,zero
	movaps	%xmm5, %xmm3
	subss	%xmm7, %xmm3
	movss	%xmm3, 320(%rsp)                # 4-byte Spill
	movss	%xmm7, 232(%rsp)                # 4-byte Spill
	movaps	%xmm1, %xmm2
	mulss	%xmm1, %xmm2
	mulss	%xmm3, %xmm3
	addss	%xmm2, %xmm3
	movss	64(%rdi), %xmm1                 # xmm1 = mem[0],zero,zero,zero
	movss	%xmm1, 236(%rsp)                # 4-byte Spill
	movaps	%xmm9, %xmm0
	subss	%xmm1, %xmm0
	movss	%xmm0, 332(%rsp)                # 4-byte Spill
	movaps	%xmm0, %xmm2
	mulss	%xmm0, %xmm2
	addss	%xmm3, %xmm2
	movaps	%xmm2, %xmm3
	mulss	%xmm2, %xmm3
	mulss	%xmm2, %xmm3
	xorps	%xmm0, %xmm0
	rsqrtss	%xmm3, %xmm0
	mulss	%xmm0, %xmm3
	mulss	%xmm0, %xmm3
	addss	%xmm13, %xmm3
	mulss	%xmm14, %xmm0
	mulss	%xmm8, %xmm0
	mulss	%xmm3, %xmm0
	movss	%xmm0, 288(%rsp)                # 4-byte Spill
	movss	28(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 216(%rsp)                # 4-byte Spill
	movaps	%xmm6, %xmm2
	subss	%xmm0, %xmm2
	movss	%xmm2, 336(%rsp)                # 4-byte Spill
	movss	48(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 220(%rsp)                # 4-byte Spill
	movaps	%xmm5, %xmm3
	subss	%xmm0, %xmm3
	movss	%xmm3, 340(%rsp)                # 4-byte Spill
	mulss	%xmm2, %xmm2
	mulss	%xmm3, %xmm3
	addss	%xmm2, %xmm3
	movss	68(%rdi), %xmm15                # xmm15 = mem[0],zero,zero,zero
	movaps	%xmm9, %xmm2
	subss	%xmm15, %xmm2
	movss	%xmm2, 344(%rsp)                # 4-byte Spill
	mulss	%xmm2, %xmm2
	addss	%xmm3, %xmm2
	movaps	%xmm2, %xmm3
	mulss	%xmm2, %xmm3
	mulss	%xmm2, %xmm3
	xorps	%xmm2, %xmm2
	rsqrtss	%xmm3, %xmm2
	mulss	%xmm2, %xmm3
	mulss	%xmm2, %xmm3
	movaps	%xmm13, %xmm1
	addss	%xmm13, %xmm3
	mulss	%xmm14, %xmm2
	mulss	%xmm8, %xmm2
	mulss	%xmm3, %xmm2
	movss	%xmm2, 284(%rsp)                # 4-byte Spill
	movss	32(%rdi), %xmm2                 # xmm2 = mem[0],zero,zero,zero
	movaps	%xmm6, %xmm4
	subss	%xmm2, %xmm4
	movss	%xmm4, 260(%rsp)                # 4-byte Spill
	movss	52(%rdi), %xmm3                 # xmm3 = mem[0],zero,zero,zero
	subss	%xmm3, %xmm5
	movss	%xmm5, 264(%rsp)                # 4-byte Spill
	mulss	%xmm4, %xmm4
	movaps	%xmm5, %xmm6
	mulss	%xmm5, %xmm6
	addss	%xmm4, %xmm6
	movss	72(%rdi), %xmm5                 # xmm5 = mem[0],zero,zero,zero
	movaps	%xmm9, %xmm4
	subss	%xmm5, %xmm4
	movss	%xmm4, 268(%rsp)                # 4-byte Spill
	mulss	%xmm4, %xmm4
	addss	%xmm6, %xmm4
	movaps	%xmm4, %xmm6
	mulss	%xmm4, %xmm6
	mulss	%xmm4, %xmm6
	xorps	%xmm4, %xmm4
	rsqrtss	%xmm6, %xmm4
	mulss	%xmm4, %xmm6
	mulss	%xmm4, %xmm6
	addss	%xmm13, %xmm6
	mulss	%xmm14, %xmm4
	mulss	%xmm8, %xmm4
	mulss	%xmm6, %xmm4
	movss	%xmm4, 280(%rsp)                # 4-byte Spill
	movaps	%xmm11, %xmm4
	subss	%xmm12, %xmm4
	movss	%xmm4, 248(%rsp)                # 4-byte Spill
	movaps	%xmm10, %xmm6
	subss	%xmm7, %xmm6
	movss	%xmm6, 292(%rsp)                # 4-byte Spill
	mulss	%xmm4, %xmm4
	mulss	%xmm6, %xmm6
	addss	%xmm4, %xmm6
	movss	240(%rsp), %xmm9                # 4-byte Reload
                                        # xmm9 = mem[0],zero,zero,zero
	movaps	%xmm9, %xmm4
	movss	236(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	subss	%xmm13, %xmm4
	movss	%xmm4, 228(%rsp)                # 4-byte Spill
	mulss	%xmm4, %xmm4
	addss	%xmm6, %xmm4
	movaps	%xmm4, %xmm7
	mulss	%xmm4, %xmm7
	mulss	%xmm4, %xmm7
	xorps	%xmm6, %xmm6
	rsqrtss	%xmm7, %xmm6
	mulss	%xmm6, %xmm7
	mulss	%xmm6, %xmm7
	addss	%xmm1, %xmm7
	mulss	%xmm14, %xmm6
	mulss	%xmm8, %xmm6
	mulss	%xmm7, %xmm6
	movaps	%xmm11, %xmm4
	movss	216(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	subss	%xmm0, %xmm4
	movss	%xmm4, 300(%rsp)                # 4-byte Spill
	movaps	%xmm10, %xmm7
	movss	220(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	subss	%xmm1, %xmm7
	movss	%xmm7, 304(%rsp)                # 4-byte Spill
	mulss	%xmm4, %xmm4
	mulss	%xmm7, %xmm7
	addss	%xmm4, %xmm7
	movaps	%xmm9, %xmm4
	movaps	%xmm9, %xmm12
	subss	%xmm15, %xmm4
	movss	%xmm4, 316(%rsp)                # 4-byte Spill
	mulss	%xmm4, %xmm4
	addss	%xmm7, %xmm4
	movaps	%xmm4, %xmm7
	mulss	%xmm4, %xmm7
	mulss	%xmm4, %xmm7
	xorps	%xmm4, %xmm4
	rsqrtss	%xmm7, %xmm4
	mulss	%xmm4, %xmm7
	mulss	%xmm4, %xmm7
	movss	.LCPI2_0(%rip), %xmm9           # xmm9 = [-3.0E+0,0.0E+0,0.0E+0,0.0E+0]
	addss	%xmm9, %xmm7
	mulss	%xmm14, %xmm4
	mulss	%xmm8, %xmm4
	mulss	%xmm7, %xmm4
	movss	%xmm4, 276(%rsp)                # 4-byte Spill
	movaps	%xmm11, %xmm4
	subss	%xmm2, %xmm4
	movss	%xmm4, 312(%rsp)                # 4-byte Spill
	subss	%xmm3, %xmm10
	movss	%xmm10, 328(%rsp)               # 4-byte Spill
	mulss	%xmm4, %xmm4
	movaps	%xmm10, %xmm7
	mulss	%xmm10, %xmm7
	addss	%xmm4, %xmm7
	movaps	%xmm12, %xmm4
	subss	%xmm5, %xmm4
	movss	%xmm4, 240(%rsp)                # 4-byte Spill
	mulss	%xmm4, %xmm4
	addss	%xmm7, %xmm4
	movaps	%xmm4, %xmm7
	mulss	%xmm4, %xmm7
	mulss	%xmm4, %xmm7
	xorps	%xmm4, %xmm4
	rsqrtss	%xmm7, %xmm4
	mulss	%xmm4, %xmm7
	mulss	%xmm4, %xmm7
	addss	%xmm9, %xmm7
	movaps	%xmm9, %xmm12
	mulss	%xmm14, %xmm4
	mulss	%xmm8, %xmm4
	mulss	%xmm7, %xmm4
	movss	%xmm4, 244(%rsp)                # 4-byte Spill
	movss	224(%rsp), %xmm10               # 4-byte Reload
                                        # xmm10 = mem[0],zero,zero,zero
	movaps	%xmm10, %xmm4
	subss	%xmm0, %xmm4
	movss	%xmm4, 364(%rsp)                # 4-byte Spill
	movss	232(%rsp), %xmm11               # 4-byte Reload
                                        # xmm11 = mem[0],zero,zero,zero
	movaps	%xmm11, %xmm9
	subss	%xmm1, %xmm9
	movss	%xmm9, 368(%rsp)                # 4-byte Spill
	mulss	%xmm4, %xmm4
	movaps	%xmm9, %xmm7
	mulss	%xmm9, %xmm7
	addss	%xmm4, %xmm7
	movaps	%xmm13, %xmm4
	subss	%xmm15, %xmm4
	movss	%xmm4, 372(%rsp)                # 4-byte Spill
	mulss	%xmm4, %xmm4
	addss	%xmm7, %xmm4
	movaps	%xmm4, %xmm7
	mulss	%xmm4, %xmm7
	mulss	%xmm4, %xmm7
	xorps	%xmm9, %xmm9
	rsqrtss	%xmm7, %xmm9
	mulss	%xmm9, %xmm7
	mulss	%xmm9, %xmm7
	addss	%xmm12, %xmm7
	mulss	%xmm14, %xmm9
	movaps	%xmm8, %xmm4
	mulss	%xmm8, %xmm9
	mulss	%xmm7, %xmm9
	movaps	%xmm10, %xmm7
	subss	%xmm2, %xmm7
	movss	%xmm7, 224(%rsp)                # 4-byte Spill
	movaps	%xmm11, %xmm10
	subss	%xmm3, %xmm10
	movss	%xmm10, 232(%rsp)               # 4-byte Spill
	mulss	%xmm7, %xmm7
	movaps	%xmm10, %xmm8
	mulss	%xmm10, %xmm8
	addss	%xmm7, %xmm8
	subss	%xmm5, %xmm13
	movss	%xmm13, 236(%rsp)               # 4-byte Spill
	mulss	%xmm13, %xmm13
	addss	%xmm8, %xmm13
	movaps	%xmm13, %xmm8
	mulss	%xmm13, %xmm8
	mulss	%xmm13, %xmm8
	xorps	%xmm7, %xmm7
	rsqrtss	%xmm8, %xmm7
	mulss	%xmm7, %xmm8
	mulss	%xmm7, %xmm8
	addss	%xmm12, %xmm8
	mulss	%xmm14, %xmm7
	mulss	%xmm4, %xmm7
	movaps	%xmm4, %xmm10
	movss	%xmm4, 388(%rsp)                # 4-byte Spill
	mulss	%xmm8, %xmm7
	movss	%xmm7, 272(%rsp)                # 4-byte Spill
	subss	%xmm2, %xmm0
	movss	%xmm0, 216(%rsp)                # 4-byte Spill
	subss	%xmm3, %xmm1
	movss	%xmm1, 220(%rsp)                # 4-byte Spill
	subss	%xmm5, %xmm15
	movss	%xmm15, 296(%rsp)               # 4-byte Spill
	mulss	%xmm0, %xmm0
	mulss	%xmm1, %xmm1
	addss	%xmm0, %xmm1
	mulss	%xmm15, %xmm15
	addss	%xmm1, %xmm15
	movaps	%xmm15, %xmm3
	mulss	%xmm15, %xmm3
	mulss	%xmm15, %xmm3
	xorps	%xmm0, %xmm0
	rsqrtss	%xmm3, %xmm0
	mulss	%xmm0, %xmm3
	mulss	%xmm0, %xmm3
	addss	%xmm12, %xmm3
	mulss	%xmm14, %xmm0
	mulss	%xmm4, %xmm0
	mulss	%xmm3, %xmm0
	movss	%xmm0, 360(%rsp)                # 4-byte Spill
	movq	m1@GOTPCREL(%rip), %rax
	movss	(%rax), %xmm1                   # xmm1 = mem[0],zero,zero,zero
	movss	252(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	movaps	%xmm13, %xmm0
	mulss	%xmm1, %xmm0
	movaps	%xmm1, %xmm10
	movss	376(%rsp), %xmm7                # 4-byte Reload
                                        # xmm7 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm0
	movss	76(%rdi), %xmm4                 # xmm4 = mem[0],zero,zero,zero
	subss	%xmm0, %xmm4
	movq	m2@GOTPCREL(%rip), %rax
	movss	(%rax), %xmm1                   # xmm1 = mem[0],zero,zero,zero
	movss	324(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm1, %xmm0
	movaps	%xmm1, %xmm15
	movss	288(%rsp), %xmm3                # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm0
	subss	%xmm0, %xmm4
	movq	m3@GOTPCREL(%rip), %rax
	movss	(%rax), %xmm1                   # xmm1 = mem[0],zero,zero,zero
	movss	336(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm1, %xmm0
	movaps	%xmm1, %xmm14
	movss	284(%rsp), %xmm8                # 4-byte Reload
                                        # xmm8 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm0
	subss	%xmm0, %xmm4
	movq	m4@GOTPCREL(%rip), %rax
	movss	(%rax), %xmm5                   # xmm5 = mem[0],zero,zero,zero
	movss	260(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm1
	movss	280(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm2, %xmm1
	subss	%xmm1, %xmm4
	movss	%xmm4, 384(%rsp)                # 4-byte Spill
	movss	256(%rsp), %xmm4                # 4-byte Reload
                                        # xmm4 = mem[0],zero,zero,zero
	movaps	%xmm4, %xmm1
	mulss	%xmm10, %xmm1
	mulss	%xmm7, %xmm1
	movss	96(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	subss	%xmm1, %xmm0
	movss	320(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm1
	mulss	%xmm3, %xmm1
	movaps	%xmm3, %xmm11
	subss	%xmm1, %xmm0
	movss	340(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm1
	movaps	%xmm8, %xmm3
	mulss	%xmm8, %xmm1
	subss	%xmm1, %xmm0
	movss	264(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm1
	mulss	%xmm2, %xmm1
	subss	%xmm1, %xmm0
	movss	%xmm0, 380(%rsp)                # 4-byte Spill
	movss	308(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm10, %xmm1
	mulss	%xmm7, %xmm1
	movss	116(%rdi), %xmm0                # xmm0 = mem[0],zero,zero,zero
	subss	%xmm1, %xmm0
	movss	332(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm1
	mulss	%xmm11, %xmm1
	subss	%xmm1, %xmm0
	movss	344(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm1
	mulss	%xmm8, %xmm1
	subss	%xmm1, %xmm0
	movss	268(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm1
	mulss	%xmm2, %xmm1
	subss	%xmm1, %xmm0
	movss	%xmm0, 356(%rsp)                # 4-byte Spill
	movq	m0@GOTPCREL(%rip), %rax
	movss	(%rax), %xmm8                   # xmm8 = mem[0],zero,zero,zero
	movaps	%xmm13, %xmm2
	mulss	%xmm8, %xmm2
	mulss	%xmm7, %xmm2
	addss	80(%rdi), %xmm2
	movss	248(%rsp), %xmm11               # 4-byte Reload
                                        # xmm11 = mem[0],zero,zero,zero
	movaps	%xmm11, %xmm13
	mulss	%xmm15, %xmm13
	mulss	%xmm6, %xmm13
	subss	%xmm13, %xmm2
	movss	300(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm13
	movss	276(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm0, %xmm13
	subss	%xmm13, %xmm2
	movss	312(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm13
	movss	244(%rsp), %xmm3                # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm13
	subss	%xmm13, %xmm2
	movss	%xmm2, 252(%rsp)                # 4-byte Spill
	mulss	%xmm8, %xmm4
	mulss	%xmm7, %xmm4
	addss	100(%rdi), %xmm4
	movss	292(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	movss	%xmm15, 348(%rsp)               # 4-byte Spill
	mulss	%xmm15, %xmm13
	mulss	%xmm6, %xmm13
	subss	%xmm13, %xmm4
	movss	304(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm13
	mulss	%xmm0, %xmm13
	subss	%xmm13, %xmm4
	movss	328(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm13
	mulss	%xmm3, %xmm13
	subss	%xmm13, %xmm4
	movss	%xmm4, 256(%rsp)                # 4-byte Spill
	movss	308(%rsp), %xmm12               # 4-byte Reload
                                        # xmm12 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm12
	mulss	%xmm7, %xmm12
	addss	120(%rdi), %xmm12
	movss	228(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm13
	mulss	%xmm6, %xmm13
	subss	%xmm13, %xmm12
	movss	316(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm13
	mulss	%xmm0, %xmm13
	subss	%xmm13, %xmm12
	movss	240(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm13
	mulss	%xmm3, %xmm13
	subss	%xmm13, %xmm12
	movss	324(%rsp), %xmm3                # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm3
	movss	288(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm2, %xmm3
	addss	84(%rdi), %xmm3
	movaps	%xmm10, %xmm1
	mulss	%xmm10, %xmm11
	mulss	%xmm6, %xmm11
	addss	%xmm3, %xmm11
	movss	364(%rsp), %xmm10               # 4-byte Reload
                                        # xmm10 = mem[0],zero,zero,zero
	movaps	%xmm10, %xmm13
	movaps	%xmm14, %xmm4
	mulss	%xmm14, %xmm13
	mulss	%xmm9, %xmm13
	subss	%xmm13, %xmm11
	movss	224(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm13
	movss	272(%rsp), %xmm15               # 4-byte Reload
                                        # xmm15 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm13
	subss	%xmm13, %xmm11
	movss	%xmm11, 248(%rsp)               # 4-byte Spill
	movss	320(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm0
	mulss	%xmm2, %xmm0
	movaps	%xmm2, %xmm11
	addss	104(%rdi), %xmm0
	movss	292(%rsp), %xmm14               # 4-byte Reload
                                        # xmm14 = mem[0],zero,zero,zero
	mulss	%xmm1, %xmm14
	mulss	%xmm6, %xmm14
	addss	%xmm0, %xmm14
	movss	368(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	movaps	%xmm2, %xmm13
	mulss	%xmm4, %xmm13
	movss	%xmm4, 352(%rsp)                # 4-byte Spill
	mulss	%xmm9, %xmm13
	subss	%xmm13, %xmm14
	movss	232(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm13
	movaps	%xmm15, %xmm7
	mulss	%xmm15, %xmm13
	subss	%xmm13, %xmm14
	movss	332(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm0
	mulss	%xmm11, %xmm0
	movss	228(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	movaps	%xmm1, %xmm3
	mulss	%xmm1, %xmm13
	mulss	%xmm6, %xmm13
	addss	124(%rdi), %xmm0
	addss	%xmm0, %xmm13
	movss	372(%rsp), %xmm11               # 4-byte Reload
                                        # xmm11 = mem[0],zero,zero,zero
	movaps	%xmm11, %xmm6
	mulss	%xmm4, %xmm6
	mulss	%xmm9, %xmm6
	subss	%xmm6, %xmm13
	movss	236(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm6
	mulss	%xmm15, %xmm6
	subss	%xmm6, %xmm13
	movss	%xmm13, 228(%rsp)               # 4-byte Spill
	movss	300(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm1, %xmm0
	movaps	%xmm1, %xmm7
	movss	276(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm1, %xmm0
	movss	348(%rsp), %xmm15               # 4-byte Reload
                                        # xmm15 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm10
	mulss	%xmm9, %xmm10
	addss	%xmm0, %xmm10
	movss	336(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm6
	movss	284(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	mulss	%xmm13, %xmm6
	addss	88(%rdi), %xmm6
	addss	%xmm6, %xmm10
	movss	216(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm6
	movss	360(%rsp), %xmm4                # 4-byte Reload
                                        # xmm4 = mem[0],zero,zero,zero
	mulss	%xmm4, %xmm6
	subss	%xmm6, %xmm10
	movss	304(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm0
	mulss	%xmm1, %xmm0
	movaps	%xmm2, %xmm3
	mulss	%xmm15, %xmm3
	mulss	%xmm9, %xmm3
	addss	%xmm0, %xmm3
	movss	340(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm6
	mulss	%xmm13, %xmm6
	addss	108(%rdi), %xmm6
	addss	%xmm6, %xmm3
	movss	220(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm6
	mulss	%xmm4, %xmm6
	subss	%xmm6, %xmm3
	movss	344(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm6
	mulss	%xmm13, %xmm6
	movss	316(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm0
	mulss	%xmm1, %xmm0
	mulss	%xmm15, %xmm11
	mulss	%xmm9, %xmm11
	addss	%xmm0, %xmm11
	addss	128(%rdi), %xmm6
	addss	%xmm6, %xmm11
	mulss	296(%rsp), %xmm5                # 4-byte Folded Reload
	mulss	%xmm4, %xmm5
	subss	%xmm5, %xmm11
	movss	312(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm0
	movss	244(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm1, %xmm0
	movss	224(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm6
	movss	272(%rsp), %xmm5                # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm6
	addss	%xmm0, %xmm6
	movss	216(%rsp), %xmm9                # 4-byte Reload
                                        # xmm9 = mem[0],zero,zero,zero
	movss	352(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm2, %xmm9
	mulss	%xmm4, %xmm9
	addss	%xmm6, %xmm9
	movss	260(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm0
	movss	280(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	mulss	%xmm13, %xmm0
	addss	92(%rdi), %xmm0
	addss	%xmm0, %xmm9
	movss	328(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm0
	mulss	%xmm1, %xmm0
	movss	232(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm6
	mulss	%xmm5, %xmm6
	addss	%xmm0, %xmm6
	movaps	%xmm6, %xmm0
	movss	220(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm2, %xmm6
	mulss	%xmm4, %xmm6
	addss	%xmm0, %xmm6
	movss	264(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm0
	mulss	%xmm13, %xmm0
	addss	112(%rdi), %xmm0
	addss	%xmm0, %xmm6
	movss	268(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm0
	mulss	%xmm13, %xmm0
	movss	240(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm1
	mulss	244(%rsp), %xmm1                # 4-byte Folded Reload
	movss	236(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm13
	mulss	%xmm5, %xmm13
	addss	%xmm1, %xmm13
	movss	296(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm2, %xmm1
	mulss	%xmm4, %xmm1
	addss	%xmm13, %xmm1
	addss	132(%rdi), %xmm0
	addss	%xmm0, %xmm1
	movss	384(%rsp), %xmm5                # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	movss	%xmm5, 76(%rdi)
	movss	380(%rsp), %xmm4                # 4-byte Reload
                                        # xmm4 = mem[0],zero,zero,zero
	movss	%xmm4, 96(%rdi)
	movss	356(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	movss	%xmm2, 116(%rdi)
	movss	252(%rsp), %xmm7                # 4-byte Reload
                                        # xmm7 = mem[0],zero,zero,zero
	movss	%xmm7, 80(%rdi)
	movss	256(%rsp), %xmm8                # 4-byte Reload
                                        # xmm8 = mem[0],zero,zero,zero
	movss	%xmm8, 100(%rdi)
	movss	%xmm12, 120(%rdi)
	movss	248(%rsp), %xmm13               # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	movss	%xmm13, 84(%rdi)
	movss	%xmm14, 104(%rdi)
	movss	228(%rsp), %xmm15               # 4-byte Reload
                                        # xmm15 = mem[0],zero,zero,zero
	movss	%xmm15, 124(%rdi)
	movss	%xmm10, 88(%rdi)
	movss	%xmm3, 108(%rdi)
	movss	%xmm11, 128(%rdi)
	movss	%xmm9, 92(%rdi)
	movss	%xmm6, 112(%rdi)
	movss	%xmm1, 132(%rdi)
	movss	388(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm0, %xmm5
	addss	16(%rdi), %xmm5
	movss	%xmm5, 16(%rdi)
	mulss	%xmm0, %xmm4
	addss	36(%rdi), %xmm4
	movss	%xmm4, 36(%rdi)
	mulss	%xmm0, %xmm2
	addss	56(%rdi), %xmm2
	movss	%xmm2, 56(%rdi)
	movaps	%xmm7, %xmm4
	mulss	%xmm0, %xmm4
	addss	20(%rdi), %xmm4
	movss	%xmm4, 20(%rdi)
	movaps	%xmm8, %xmm4
	mulss	%xmm0, %xmm4
	addss	40(%rdi), %xmm4
	movss	%xmm4, 40(%rdi)
	mulss	%xmm0, %xmm12
	addss	60(%rdi), %xmm12
	movss	%xmm12, 60(%rdi)
	movaps	%xmm13, %xmm4
	mulss	%xmm0, %xmm4
	addss	24(%rdi), %xmm4
	movss	%xmm4, 24(%rdi)
	mulss	%xmm0, %xmm14
	addss	44(%rdi), %xmm14
	movss	%xmm14, 44(%rdi)
	movaps	%xmm15, %xmm4
	mulss	%xmm0, %xmm4
	addss	64(%rdi), %xmm4
	movss	%xmm4, 64(%rdi)
	mulss	%xmm0, %xmm10
	addss	28(%rdi), %xmm10
	movss	%xmm10, 28(%rdi)
	mulss	%xmm0, %xmm3
	addss	48(%rdi), %xmm3
	movss	%xmm3, 48(%rdi)
	mulss	%xmm0, %xmm11
	addss	68(%rdi), %xmm11
	movss	%xmm11, 68(%rdi)
	mulss	%xmm0, %xmm9
	addss	32(%rdi), %xmm9
	movss	%xmm9, 32(%rdi)
	mulss	%xmm0, %xmm6
	addss	52(%rdi), %xmm6
	movss	%xmm6, 52(%rdi)
	mulss	%xmm0, %xmm1
	addss	72(%rdi), %xmm1
	movss	%xmm1, 72(%rdi)
	movq	8(%rdi), %rax
	incq	%rax
	movq	%rax, 8(%rdi)
	cmpq	(%rdi), %rax
	jne	.LBB2_4
# %bb.3:                                # %guard.then974
	movss	16(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 240(%rsp)                # 4-byte Spill
	movss	20(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 236(%rsp)                # 4-byte Spill
	movss	24(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 232(%rsp)                # 4-byte Spill
	movss	28(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 228(%rsp)                # 4-byte Spill
	movss	32(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 224(%rsp)                # 4-byte Spill
	movss	36(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 220(%rsp)                # 4-byte Spill
	movss	40(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 216(%rsp)                # 4-byte Spill
	movss	44(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 256(%rsp)                # 4-byte Spill
	movss	48(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 252(%rsp)                # 4-byte Spill
	movss	52(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 248(%rsp)                # 4-byte Spill
	movss	56(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 244(%rsp)                # 4-byte Spill
	movss	60(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 268(%rsp)                # 4-byte Spill
	movss	64(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 264(%rsp)                # 4-byte Spill
	movss	68(%rdi), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 260(%rsp)                # 4-byte Spill
	movss	72(%rdi), %xmm14                # xmm14 = mem[0],zero,zero,zero
	movss	76(%rdi), %xmm15                # xmm15 = mem[0],zero,zero,zero
	movss	80(%rdi), %xmm13                # xmm13 = mem[0],zero,zero,zero
	movss	84(%rdi), %xmm12                # xmm12 = mem[0],zero,zero,zero
	movss	88(%rdi), %xmm11                # xmm11 = mem[0],zero,zero,zero
	movss	92(%rdi), %xmm10                # xmm10 = mem[0],zero,zero,zero
	movss	96(%rdi), %xmm9                 # xmm9 = mem[0],zero,zero,zero
	movss	100(%rdi), %xmm8                # xmm8 = mem[0],zero,zero,zero
	movss	104(%rdi), %xmm7                # xmm7 = mem[0],zero,zero,zero
	movss	108(%rdi), %xmm6                # xmm6 = mem[0],zero,zero,zero
	movss	112(%rdi), %xmm5                # xmm5 = mem[0],zero,zero,zero
	movss	116(%rdi), %xmm4                # xmm4 = mem[0],zero,zero,zero
	movss	120(%rdi), %xmm3                # xmm3 = mem[0],zero,zero,zero
	movss	124(%rdi), %xmm2                # xmm2 = mem[0],zero,zero,zero
	movss	128(%rdi), %xmm1                # xmm1 = mem[0],zero,zero,zero
	movss	132(%rdi), %xmm0                # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 208(%rsp)
	movss	%xmm1, 200(%rsp)
	movss	%xmm2, 192(%rsp)
	movss	%xmm3, 184(%rsp)
	movss	%xmm4, 176(%rsp)
	movss	%xmm5, 168(%rsp)
	movss	%xmm6, 160(%rsp)
	movss	%xmm7, 152(%rsp)
	movss	%xmm8, 144(%rsp)
	movss	%xmm9, 136(%rsp)
	movss	%xmm10, 128(%rsp)
	movss	%xmm11, 120(%rsp)
	movss	%xmm12, 112(%rsp)
	movss	%xmm13, 104(%rsp)
	movss	%xmm15, 96(%rsp)
	movss	%xmm14, 48(%rsp)
	movss	260(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 40(%rsp)
	movss	264(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 32(%rsp)
	movss	268(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 24(%rsp)
	movss	244(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 16(%rsp)
	movss	248(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 8(%rsp)
	movss	252(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, (%rsp)
	movl	$990201755, 88(%rsp)            # imm = 0x3B05479B
	movl	$987885205, 80(%rsp)            # imm = 0x3AE1EE95
	movl	$1010362952, 72(%rsp)           # imm = 0x3C38EA48
	movl	$1025139887, 64(%rsp)           # imm = 0x3D1A64AF
	movl	$1109256678, 56(%rsp)           # imm = 0x421DE9E6
	movss	240(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	236(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	movss	232(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	movss	228(%rsp), %xmm3                # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	movss	224(%rsp), %xmm4                # 4-byte Reload
                                        # xmm4 = mem[0],zero,zero,zero
	movss	220(%rsp), %xmm5                # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	movss	216(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	movss	256(%rsp), %xmm7                # 4-byte Reload
                                        # xmm7 = mem[0],zero,zero,zero
	callq	txn_simulate_cold_0@PLT
.LBB2_4:                                # %guard.end974
	addq	$392, %rsp                      # imm = 0x188
	retq
.LBB2_1:                                # %pp13
.Lfunc_end2:
	.size	txn_simulate, .Lfunc_end2-txn_simulate
                                        # -- End function
	.section	.rodata.cst4,"aM",@progbits,4
	.p2align	2, 0x0                          # -- Begin function txn_simulate_cold_0
.LCPI3_0:
	.long	0xc0400000                      # float -3
.LCPI3_1:
	.long	0xbf000000                      # float -0.5
.LCPI3_2:
	.long	0x3f000000                      # float 0.5
	.text
	.globl	txn_simulate_cold_0
	.p2align	4, 0x90
	.type	txn_simulate_cold_0,@function
txn_simulate_cold_0:                    # @txn_simulate_cold_0
# %bb.0:
	subq	$24, %rsp
	movaps	%xmm7, %xmm8
	movaps	%xmm6, %xmm7
	movaps	%xmm5, %xmm6
	movss	%xmm4, 8(%rsp)                  # 4-byte Spill
	movaps	%xmm3, %xmm5
	movss	%xmm3, (%rsp)                   # 4-byte Spill
	movaps	%xmm2, %xmm13
	movss	%xmm2, 4(%rsp)                  # 4-byte Spill
	movaps	%xmm1, %xmm4
	movaps	%xmm0, %xmm3
	movss	64(%rsp), %xmm11                # xmm11 = mem[0],zero,zero,zero
	movss	56(%rsp), %xmm2                 # xmm2 = mem[0],zero,zero,zero
	movss	48(%rsp), %xmm0                 # xmm0 = mem[0],zero,zero,zero
	movaps	%xmm3, %xmm9
	subss	%xmm1, %xmm9
	mulss	%xmm9, %xmm9
	movaps	%xmm6, %xmm10
	subss	%xmm7, %xmm10
	mulss	%xmm10, %xmm10
	addss	%xmm9, %xmm10
	movaps	%xmm0, %xmm12
	subss	%xmm2, %xmm12
	mulss	%xmm12, %xmm12
	addss	%xmm10, %xmm12
	xorps	%xmm9, %xmm9
	rsqrtss	%xmm12, %xmm9
	mulss	%xmm9, %xmm12
	mulss	%xmm9, %xmm12
	movss	.LCPI3_0(%rip), %xmm1           # xmm1 = [-3.0E+0,0.0E+0,0.0E+0,0.0E+0]
	addss	%xmm1, %xmm12
	movaps	%xmm1, %xmm15
	movss	.LCPI3_1(%rip), %xmm1           # xmm1 = [-5.0E-1,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm1, %xmm9
	movaps	%xmm1, %xmm10
	mulss	%xmm12, %xmm9
	movss	%xmm9, 16(%rsp)                 # 4-byte Spill
	movaps	%xmm3, %xmm12
	subss	%xmm13, %xmm12
	mulss	%xmm12, %xmm12
	movaps	%xmm6, %xmm13
	subss	%xmm8, %xmm13
	movss	%xmm8, 12(%rsp)                 # 4-byte Spill
	mulss	%xmm13, %xmm13
	addss	%xmm12, %xmm13
	movaps	%xmm0, %xmm14
	subss	%xmm11, %xmm14
	mulss	%xmm14, %xmm14
	addss	%xmm13, %xmm14
	xorps	%xmm12, %xmm12
	rsqrtss	%xmm14, %xmm12
	mulss	%xmm12, %xmm14
	mulss	%xmm12, %xmm14
	movaps	%xmm15, %xmm9
	addss	%xmm15, %xmm14
	mulss	%xmm1, %xmm12
	mulss	%xmm14, %xmm12
	movaps	%xmm3, %xmm13
	subss	%xmm5, %xmm13
	mulss	%xmm13, %xmm13
	movss	32(%rsp), %xmm15                # xmm15 = mem[0],zero,zero,zero
	movaps	%xmm6, %xmm5
	subss	%xmm15, %xmm5
	mulss	%xmm5, %xmm5
	addss	%xmm13, %xmm5
	movss	72(%rsp), %xmm14                # xmm14 = mem[0],zero,zero,zero
	movaps	%xmm0, %xmm1
	subss	%xmm14, %xmm1
	mulss	%xmm1, %xmm1
	addss	%xmm5, %xmm1
	xorps	%xmm13, %xmm13
	rsqrtss	%xmm1, %xmm13
	mulss	%xmm13, %xmm1
	mulss	%xmm13, %xmm1
	addss	%xmm9, %xmm1
	movaps	%xmm10, %xmm5
	mulss	%xmm10, %xmm13
	mulss	%xmm1, %xmm13
	movss	8(%rsp), %xmm10                 # 4-byte Reload
                                        # xmm10 = mem[0],zero,zero,zero
	subss	%xmm10, %xmm3
	mulss	%xmm3, %xmm3
	subss	40(%rsp), %xmm6
	mulss	%xmm6, %xmm6
	addss	%xmm3, %xmm6
	subss	80(%rsp), %xmm0
	mulss	%xmm0, %xmm0
	addss	%xmm6, %xmm0
	xorps	%xmm1, %xmm1
	rsqrtss	%xmm0, %xmm1
	mulss	%xmm1, %xmm0
	mulss	%xmm1, %xmm0
	movaps	%xmm9, %xmm6
	addss	%xmm9, %xmm0
	mulss	%xmm5, %xmm1
	movaps	%xmm5, %xmm9
	mulss	%xmm0, %xmm1
	movss	%xmm1, 20(%rsp)                 # 4-byte Spill
	movaps	%xmm4, %xmm0
	subss	4(%rsp), %xmm0                  # 4-byte Folded Reload
	mulss	%xmm0, %xmm0
	movaps	%xmm7, %xmm1
	subss	%xmm8, %xmm1
	mulss	%xmm1, %xmm1
	addss	%xmm0, %xmm1
	movaps	%xmm2, %xmm0
	subss	%xmm11, %xmm0
	mulss	%xmm0, %xmm0
	addss	%xmm1, %xmm0
	xorps	%xmm5, %xmm5
	rsqrtss	%xmm0, %xmm5
	mulss	%xmm5, %xmm0
	mulss	%xmm5, %xmm0
	addss	%xmm6, %xmm0
	movaps	%xmm6, %xmm8
	mulss	%xmm9, %xmm5
	mulss	%xmm0, %xmm5
	movaps	%xmm4, %xmm0
	movss	(%rsp), %xmm3                   # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	subss	%xmm3, %xmm0
	mulss	%xmm0, %xmm0
	movaps	%xmm7, %xmm1
	subss	%xmm15, %xmm1
	mulss	%xmm1, %xmm1
	addss	%xmm0, %xmm1
	movaps	%xmm2, %xmm0
	subss	%xmm14, %xmm0
	mulss	%xmm0, %xmm0
	addss	%xmm1, %xmm0
	xorps	%xmm1, %xmm1
	rsqrtss	%xmm0, %xmm1
	mulss	%xmm1, %xmm0
	mulss	%xmm1, %xmm0
	addss	%xmm6, %xmm0
	mulss	%xmm9, %xmm1
	mulss	%xmm0, %xmm1
	subss	%xmm10, %xmm4
	mulss	%xmm4, %xmm4
	subss	40(%rsp), %xmm7
	mulss	%xmm7, %xmm7
	addss	%xmm4, %xmm7
	subss	80(%rsp), %xmm2
	mulss	%xmm2, %xmm2
	addss	%xmm7, %xmm2
	xorps	%xmm6, %xmm6
	rsqrtss	%xmm2, %xmm6
	mulss	%xmm6, %xmm2
	mulss	%xmm6, %xmm2
	addss	%xmm8, %xmm2
	movaps	%xmm8, %xmm10
	mulss	%xmm9, %xmm6
	mulss	%xmm2, %xmm6
	movss	4(%rsp), %xmm7                  # 4-byte Reload
                                        # xmm7 = mem[0],zero,zero,zero
	movaps	%xmm7, %xmm0
	subss	%xmm3, %xmm0
	mulss	%xmm0, %xmm0
	movss	12(%rsp), %xmm3                 # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	movaps	%xmm3, %xmm2
	subss	%xmm15, %xmm2
	mulss	%xmm2, %xmm2
	addss	%xmm0, %xmm2
	movaps	%xmm11, %xmm0
	subss	%xmm14, %xmm0
	mulss	%xmm0, %xmm0
	addss	%xmm2, %xmm0
	xorps	%xmm2, %xmm2
	rsqrtss	%xmm0, %xmm2
	mulss	%xmm2, %xmm0
	mulss	%xmm2, %xmm0
	addss	%xmm8, %xmm0
	mulss	%xmm9, %xmm2
	movaps	%xmm9, %xmm8
	mulss	%xmm0, %xmm2
	movss	8(%rsp), %xmm4                  # 4-byte Reload
                                        # xmm4 = mem[0],zero,zero,zero
	movaps	%xmm7, %xmm0
	subss	%xmm4, %xmm0
	mulss	%xmm0, %xmm0
	movss	40(%rsp), %xmm9                 # xmm9 = mem[0],zero,zero,zero
	subss	%xmm9, %xmm3
	mulss	%xmm3, %xmm3
	addss	%xmm0, %xmm3
	movaps	%xmm3, %xmm0
	movss	80(%rsp), %xmm3                 # xmm3 = mem[0],zero,zero,zero
	subss	%xmm3, %xmm11
	mulss	%xmm11, %xmm11
	addss	%xmm0, %xmm11
	xorps	%xmm7, %xmm7
	rsqrtss	%xmm11, %xmm7
	mulss	%xmm7, %xmm11
	mulss	%xmm7, %xmm11
	addss	%xmm10, %xmm11
	mulss	%xmm8, %xmm7
	mulss	%xmm11, %xmm7
	movss	(%rsp), %xmm0                   # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	subss	%xmm4, %xmm0
	subss	%xmm9, %xmm15
	mulss	%xmm0, %xmm0
	mulss	%xmm15, %xmm15
	addss	%xmm0, %xmm15
	subss	%xmm3, %xmm14
	movss	104(%rsp), %xmm0                # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm14
	addss	%xmm15, %xmm14
	xorps	%xmm11, %xmm11
	rsqrtss	%xmm14, %xmm11
	mulss	%xmm11, %xmm14
	mulss	%xmm11, %xmm14
	addss	%xmm10, %xmm14
	movss	96(%rsp), %xmm8                 # xmm8 = mem[0],zero,zero,zero
	mulss	.LCPI3_1(%rip), %xmm11
	movss	88(%rsp), %xmm10                # xmm10 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm11
	movaps	%xmm10, %xmm14
	mulss	%xmm8, %xmm14
	mulss	16(%rsp), %xmm14                # 4-byte Folded Reload
	movaps	%xmm10, %xmm15
	mulss	%xmm0, %xmm15
	mulss	%xmm12, %xmm15
	movss	112(%rsp), %xmm4                # xmm4 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm15
	movaps	%xmm10, %xmm12
	mulss	%xmm4, %xmm12
	mulss	%xmm13, %xmm12
	movss	120(%rsp), %xmm9                # xmm9 = mem[0],zero,zero,zero
	addss	%xmm15, %xmm12
	movaps	%xmm10, %xmm13
	mulss	%xmm9, %xmm13
	mulss	20(%rsp), %xmm13                # 4-byte Folded Reload
	movaps	%xmm8, %xmm3
	mulss	%xmm0, %xmm3
	mulss	%xmm5, %xmm3
	addss	%xmm13, %xmm3
	addss	%xmm12, %xmm3
	movaps	%xmm8, %xmm5
	mulss	%xmm4, %xmm5
	mulss	%xmm1, %xmm5
	movaps	%xmm8, %xmm1
	mulss	%xmm9, %xmm1
	mulss	%xmm6, %xmm1
	addss	%xmm5, %xmm1
	movaps	%xmm0, %xmm5
	mulss	%xmm4, %xmm5
	mulss	%xmm2, %xmm5
	addss	%xmm1, %xmm5
	addss	%xmm3, %xmm5
	movaps	%xmm0, %xmm1
	mulss	%xmm9, %xmm1
	mulss	%xmm7, %xmm1
	movaps	%xmm4, %xmm3
	mulss	%xmm9, %xmm3
	mulss	%xmm11, %xmm3
	addss	%xmm1, %xmm3
	movss	168(%rsp), %xmm2                # xmm2 = mem[0],zero,zero,zero
	addss	%xmm5, %xmm3
	movss	128(%rsp), %xmm1                # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm1, %xmm1
	mulss	%xmm2, %xmm2
	addss	%xmm1, %xmm2
	movss	208(%rsp), %xmm1                # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm1, %xmm1
	addss	%xmm2, %xmm1
	movss	.LCPI3_2(%rip), %xmm2           # xmm2 = [5.0E-1,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm2, %xmm10
	mulss	%xmm10, %xmm1
	movss	176(%rsp), %xmm5                # xmm5 = mem[0],zero,zero,zero
	subss	%xmm3, %xmm1
	movss	136(%rsp), %xmm3                # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm3
	mulss	%xmm5, %xmm5
	addss	%xmm3, %xmm5
	movss	216(%rsp), %xmm3                # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm3
	addss	%xmm5, %xmm3
	movss	184(%rsp), %xmm5                # xmm5 = mem[0],zero,zero,zero
	mulss	%xmm2, %xmm8
	mulss	%xmm8, %xmm3
	movss	144(%rsp), %xmm6                # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm6, %xmm6
	mulss	%xmm5, %xmm5
	addss	%xmm6, %xmm5
	movss	224(%rsp), %xmm6                # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm6, %xmm6
	addss	%xmm5, %xmm6
	mulss	%xmm2, %xmm0
	mulss	%xmm0, %xmm6
	movss	192(%rsp), %xmm0                # xmm0 = mem[0],zero,zero,zero
	addss	%xmm3, %xmm6
	movss	152(%rsp), %xmm3                # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm3
	mulss	%xmm0, %xmm0
	addss	%xmm3, %xmm0
	movss	232(%rsp), %xmm3                # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm3
	addss	%xmm0, %xmm3
	mulss	%xmm2, %xmm4
	mulss	%xmm4, %xmm3
	addss	%xmm6, %xmm3
	movss	200(%rsp), %xmm4                # xmm4 = mem[0],zero,zero,zero
	mulss	%xmm2, %xmm9
	movss	160(%rsp), %xmm0                # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm0, %xmm0
	mulss	%xmm4, %xmm4
	addss	%xmm0, %xmm4
	movss	240(%rsp), %xmm0                # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm0, %xmm0
	addss	%xmm4, %xmm0
	mulss	%xmm9, %xmm0
	addss	%xmm3, %xmm0
	addss	%xmm1, %xmm0
	callq	__print_float@PLT
	movl	$10, %edi
	callq	__print_char@PLT
	addq	$24, %rsp
	retq
.Lfunc_end3:
	.size	txn_simulate_cold_0, .Lfunc_end3-txn_simulate_cold_0
                                        # -- End function
	.p2align	4, 0x90                         # -- Begin function pre_simulate
	.type	pre_simulate,@function
pre_simulate:                           # @pre_simulate
# %bb.0:                                # %entry
	movq	8(%rdi), %rax
	cmpq	(%rdi), %rax
	setl	%al
	retq
.Lfunc_end4:
	.size	pre_simulate, .Lfunc_end4-pre_simulate
                                        # -- End function
	.section	.rodata.cst4,"aM",@progbits,4
	.p2align	2, 0x0                          # -- Begin function init_state
.LCPI5_0:
	.long	0x3ad996ee                      # float 0.00166007667
.LCPI5_1:
	.long	0xbb355db0                      # float -0.00276742503
.LCPI5_2:
	.long	0x3b4249c2                      # float 0.00296460139
.LCPI5_3:
	.long	0x3b2fae4f                      # float 0.00268067769
.LCPI5_4:
	.long	0x3bfc47fd                      # float 0.00769901136
.LCPI5_5:
	.long	0x3ba3cab1                      # float 0.00499852793
.LCPI5_6:
	.long	0x3b1be022                      # float 0.00237847166
.LCPI5_7:
	.long	0x3ad56aba                      # float 0.00162824173
.LCPI5_8:
	.long	0xb890ccca                      # float -6.90460001E-5
.LCPI5_9:
	.long	0x37c149bd                      # float 2.30417299E-5
.LCPI5_10:
	.long	0xb7f8cc20                      # float -2.96589569E-5
.LCPI5_11:
	.long	0xb8c79038                      # float -9.51592228E-5
	.text
	.globl	init_state
	.p2align	4, 0x90
	.type	init_state,@function
init_state:                             # @init_state
# %bb.0:                                # %entry
	pushq	%rbx
	movq	%rdi, %rbx
	movl	$.Lstr.1, %esi
	callq	get_env_int@PLT
	movq	%rax, (%rbx)
	movq	$0, 8(%rbx)
	movabsq	$4655293757686546432, %rax      # imm = 0x409AED0200000000
	movq	%rax, 16(%rbx)
	movabsq	$4705785892525407854, %rax      # imm = 0x414E4F5641057E6E
	movq	%rax, 24(%rbx)
	movq	$1098257213, 32(%rbx)           # imm = 0x4176133D
	movabsq	$4648838906091177310, %rax      # imm = 0x4083FE5ABF94855E
	movq	%rax, 40(%rbx)
	movabsq	$-4481263311694739641, %rax     # imm = 0xC1CF5AC2C171C747
	movq	%rax, 48(%rbx)
	movabsq	$-4768124760460623872, %rax     # imm = 0xBDD437CB00000000
	movq	%rax, 56(%rbx)
	movabsq	$-4727465972610458977, %rax     # imm = 0xBE64AABEBECE9A9F
	movq	%rax, 64(%rbx)
	movq	$1043828637, 72(%rbx)           # imm = 0x3E378F9D
	movq	dpy@GOTPCREL(%rip), %rax
	movss	(%rax), %xmm0                   # xmm0 = mem[0],zero,zero,zero
	movss	.LCPI5_0(%rip), %xmm1           # xmm1 = [1.66007667E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm0, %xmm1
	movss	%xmm1, 80(%rbx)
	movss	.LCPI5_1(%rip), %xmm1           # xmm1 = [-2.76742503E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm0, %xmm1
	movss	%xmm1, 84(%rbx)
	movss	.LCPI5_2(%rip), %xmm1           # xmm1 = [2.96460139E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm0, %xmm1
	movss	%xmm1, 88(%rbx)
	movss	.LCPI5_3(%rip), %xmm1           # xmm1 = [2.68067769E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm0, %xmm1
	movss	%xmm1, 92(%rbx)
	movl	$0, 96(%rbx)
	movss	.LCPI5_4(%rip), %xmm1           # xmm1 = [7.69901136E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm0, %xmm1
	movss	%xmm1, 100(%rbx)
	movss	.LCPI5_5(%rip), %xmm1           # xmm1 = [4.99852793E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm0, %xmm1
	movss	%xmm1, 104(%rbx)
	movss	.LCPI5_6(%rip), %xmm1           # xmm1 = [2.37847166E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm0, %xmm1
	movss	%xmm1, 108(%rbx)
	movss	.LCPI5_7(%rip), %xmm1           # xmm1 = [1.62824173E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm0, %xmm1
	movss	%xmm1, 112(%rbx)
	movl	$0, 116(%rbx)
	movss	.LCPI5_8(%rip), %xmm1           # xmm1 = [-6.90460001E-5,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm0, %xmm1
	movss	%xmm1, 120(%rbx)
	movss	.LCPI5_9(%rip), %xmm1           # xmm1 = [2.30417299E-5,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm0, %xmm1
	movss	%xmm1, 124(%rbx)
	movss	.LCPI5_10(%rip), %xmm1          # xmm1 = [-2.96589569E-5,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm0, %xmm1
	movss	%xmm1, 128(%rbx)
	mulss	.LCPI5_11(%rip), %xmm0
	movss	%xmm0, 132(%rbx)
	movq	$0, 136(%rbx)
	popq	%rbx
	retq
.Lfunc_end5:
	.size	init_state, .Lfunc_end5-init_state
                                        # -- End function
	.section	.rodata.cst4,"aM",@progbits,4
	.p2align	2, 0x0                          # -- Begin function main
.LCPI6_0:
	.long	0x3ad996ee                      # float 0.00166007667
.LCPI6_1:
	.long	0xbb355db0                      # float -0.00276742503
.LCPI6_2:
	.long	0x3b4249c2                      # float 0.00296460139
.LCPI6_3:
	.long	0x3b2fae4f                      # float 0.00268067769
.LCPI6_4:
	.long	0x3bfc47fd                      # float 0.00769901136
.LCPI6_5:
	.long	0x3ba3cab1                      # float 0.00499852793
.LCPI6_6:
	.long	0x3b1be022                      # float 0.00237847166
.LCPI6_7:
	.long	0x3ad56aba                      # float 0.00162824173
.LCPI6_8:
	.long	0xb890ccca                      # float -6.90460001E-5
.LCPI6_9:
	.long	0x37c149bd                      # float 2.30417299E-5
.LCPI6_10:
	.long	0xb7f8cc20                      # float -2.96589569E-5
.LCPI6_11:
	.long	0xb8c79038                      # float -9.51592228E-5
.LCPI6_12:
	.long	0xbece9a9f                      # float -0.403523415
.LCPI6_13:
	.long	0xbe64aabe                      # float -0.22330758
.LCPI6_14:
	.long	0x3e378f9d                      # float 0.179258779
.LCPI6_15:
	.long	0xc0400000                      # float -3
.LCPI6_16:
	.long	0xbf000000                      # float -0.5
.LCPI6_17:
	.long	0x3f000000                      # float 0.5
	.text
	.globl	main
	.p2align	4, 0x90
	.type	main,@function
main:                                   # @main
# %bb.0:                                # %entry
	pushq	%rbx
	subq	$416, %rsp                      # imm = 0x1A0
	leaq	240(%rsp), %rdi
	movl	$.Lstr.1, %esi
	callq	get_env_int@PLT
	movq	%rax, 240(%rsp)
	movq	$0, 248(%rsp)
	movabsq	$4655293757686546432, %rax      # imm = 0x409AED0200000000
	movq	%rax, 256(%rsp)
	movabsq	$4705785892525407854, %rax      # imm = 0x414E4F5641057E6E
	movq	%rax, 264(%rsp)
	movq	$1098257213, 272(%rsp)          # imm = 0x4176133D
	movabsq	$4648838906091177310, %rax      # imm = 0x4083FE5ABF94855E
	movq	%rax, 280(%rsp)
	movabsq	$-4481263311694739641, %rax     # imm = 0xC1CF5AC2C171C747
	movq	%rax, 288(%rsp)
	movabsq	$-4768124760460623872, %rax     # imm = 0xBDD437CB00000000
	movq	%rax, 296(%rsp)
	movabsq	$-4727465972610458977, %rax     # imm = 0xBE64AABEBECE9A9F
	movq	%rax, 304(%rsp)
	movq	$1043828637, 312(%rsp)          # imm = 0x3E378F9D
	movq	dpy@GOTPCREL(%rip), %rax
	movss	(%rax), %xmm1                   # xmm1 = mem[0],zero,zero,zero
	movss	.LCPI6_0(%rip), %xmm0           # xmm0 = [1.66007667E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm1, %xmm0
	movss	%xmm0, 52(%rsp)                 # 4-byte Spill
	movss	%xmm0, 320(%rsp)
	movss	.LCPI6_1(%rip), %xmm0           # xmm0 = [-2.76742503E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm1, %xmm0
	movss	%xmm0, 116(%rsp)                # 4-byte Spill
	movss	%xmm0, 324(%rsp)
	movss	.LCPI6_2(%rip), %xmm0           # xmm0 = [2.96460139E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm1, %xmm0
	movss	%xmm0, 128(%rsp)                # 4-byte Spill
	movss	%xmm0, 328(%rsp)
	movss	.LCPI6_3(%rip), %xmm0           # xmm0 = [2.68067769E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm1, %xmm0
	movss	%xmm0, 140(%rsp)                # 4-byte Spill
	movss	%xmm0, 332(%rsp)
	movl	$0, 336(%rsp)
	movss	.LCPI6_4(%rip), %xmm0           # xmm0 = [7.69901136E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm1, %xmm0
	movss	%xmm0, 48(%rsp)                 # 4-byte Spill
	movss	%xmm0, 340(%rsp)
	movss	.LCPI6_5(%rip), %xmm0           # xmm0 = [4.99852793E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm1, %xmm0
	movss	%xmm0, 120(%rsp)                # 4-byte Spill
	movss	%xmm0, 344(%rsp)
	movss	.LCPI6_6(%rip), %xmm0           # xmm0 = [2.37847166E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm1, %xmm0
	movss	%xmm0, 132(%rsp)                # 4-byte Spill
	movss	%xmm0, 348(%rsp)
	movss	.LCPI6_7(%rip), %xmm0           # xmm0 = [1.62824173E-3,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm1, %xmm0
	movss	%xmm0, 144(%rsp)                # 4-byte Spill
	movss	%xmm0, 352(%rsp)
	movl	$0, 356(%rsp)
	movss	.LCPI6_8(%rip), %xmm0           # xmm0 = [-6.90460001E-5,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm1, %xmm0
	movss	%xmm0, 76(%rsp)                 # 4-byte Spill
	movss	%xmm0, 360(%rsp)
	movss	.LCPI6_9(%rip), %xmm0           # xmm0 = [2.30417299E-5,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm1, %xmm0
	movss	%xmm0, 124(%rsp)                # 4-byte Spill
	movss	%xmm0, 364(%rsp)
	movss	.LCPI6_10(%rip), %xmm0          # xmm0 = [-2.96589569E-5,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm1, %xmm0
	movss	%xmm0, 136(%rsp)                # 4-byte Spill
	movss	%xmm0, 368(%rsp)
	mulss	.LCPI6_11(%rip), %xmm1
	movss	%xmm1, 148(%rsp)                # 4-byte Spill
	movss	%xmm1, 372(%rsp)
	movq	$0, 376(%rsp)
	movq	240(%rsp), %r8
	movq	248(%rsp), %rbx
	movss	256(%rsp), %xmm11               # xmm11 = mem[0],zero,zero,zero
	movss	260(%rsp), %xmm14               # xmm14 = mem[0],zero,zero,zero
	movss	264(%rsp), %xmm0                # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 12(%rsp)                 # 4-byte Spill
	movss	268(%rsp), %xmm0                # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 80(%rsp)                 # 4-byte Spill
	movq	dt@GOTPCREL(%rip), %r9
	movq	m1@GOTPCREL(%rip), %rsi
	movq	m2@GOTPCREL(%rip), %rdx
	movq	m3@GOTPCREL(%rip), %rcx
	movq	m4@GOTPCREL(%rip), %rax
	movq	m0@GOTPCREL(%rip), %rdi
	movss	272(%rsp), %xmm0                # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 92(%rsp)                 # 4-byte Spill
	movss	276(%rsp), %xmm12               # xmm12 = mem[0],zero,zero,zero
	movss	280(%rsp), %xmm9                # xmm9 = mem[0],zero,zero,zero
	movss	284(%rsp), %xmm7                # xmm7 = mem[0],zero,zero,zero
	movss	288(%rsp), %xmm0                # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 84(%rsp)                 # 4-byte Spill
	movss	292(%rsp), %xmm0                # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 72(%rsp)                 # 4-byte Spill
	movss	296(%rsp), %xmm10               # xmm10 = mem[0],zero,zero,zero
	movss	300(%rsp), %xmm0                # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 20(%rsp)                 # 4-byte Spill
	movss	.LCPI6_12(%rip), %xmm13         # xmm13 = [-4.03523415E-1,0.0E+0,0.0E+0,0.0E+0]
	movss	.LCPI6_13(%rip), %xmm0          # xmm0 = [-2.2330758E-1,0.0E+0,0.0E+0,0.0E+0]
	movss	%xmm0, 88(%rsp)                 # 4-byte Spill
	movss	.LCPI6_14(%rip), %xmm0          # xmm0 = [1.79258779E-1,0.0E+0,0.0E+0,0.0E+0]
	movss	%xmm0, 96(%rsp)                 # 4-byte Spill
	xorps	%xmm0, %xmm0
	movss	%xmm0, 40(%rsp)                 # 4-byte Spill
	movss	%xmm0, 36(%rsp)                 # 4-byte Spill
	movss	%xmm0, 44(%rsp)                 # 4-byte Spill
	.p2align	4, 0x90
.LBB6_1:                                # %.cm_header
                                        # =>This Inner Loop Header: Depth=1
	cmpq	%r8, %rbx
	movaps	%xmm11, %xmm0
	subss	%xmm14, %xmm0
	movaps	%xmm12, %xmm6
	movss	%xmm9, 16(%rsp)                 # 4-byte Spill
	subss	%xmm9, %xmm12
	movaps	%xmm10, %xmm4
	movaps	%xmm10, %xmm2
	subss	20(%rsp), %xmm2                 # 4-byte Folded Reload
	jge	.LBB6_3
# %bb.2:                                # %.cm_body
                                        #   in Loop: Header=BB6_1 Depth=1
	movaps	%xmm14, %xmm3
	movss	%xmm0, 176(%rsp)                # 4-byte Spill
	mulss	%xmm0, %xmm0
	movaps	%xmm12, %xmm1
	movss	%xmm12, 388(%rsp)               # 4-byte Spill
	mulss	%xmm12, %xmm1
	addss	%xmm0, %xmm1
	movaps	%xmm2, %xmm0
	mulss	%xmm2, %xmm0
	addss	%xmm1, %xmm0
	movaps	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	xorps	%xmm9, %xmm9
	rsqrtss	%xmm1, %xmm9
	mulss	%xmm9, %xmm1
	mulss	%xmm9, %xmm1
	movss	.LCPI6_15(%rip), %xmm5          # xmm5 = [-3.0E+0,0.0E+0,0.0E+0,0.0E+0]
	addss	%xmm5, %xmm1
	movaps	%xmm7, %xmm0
	movss	.LCPI6_16(%rip), %xmm7          # xmm7 = [-5.0E-1,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm7, %xmm9
	movaps	%xmm7, %xmm8
	movss	(%r9), %xmm7                    # xmm7 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm9
	movaps	%xmm7, %xmm15
	mulss	%xmm1, %xmm9
	movss	%xmm9, 396(%rsp)                # 4-byte Spill
	movaps	%xmm11, %xmm7
	movss	12(%rsp), %xmm12                # 4-byte Reload
                                        # xmm12 = mem[0],zero,zero,zero
	subss	%xmm12, %xmm7
	movss	%xmm7, 212(%rsp)                # 4-byte Spill
	movss	%xmm12, 12(%rsp)                # 4-byte Spill
	movaps	%xmm6, %xmm1
	subss	%xmm0, %xmm1
	movss	%xmm1, 216(%rsp)                # 4-byte Spill
	movaps	%xmm0, %xmm9
	movss	%xmm0, 228(%rsp)                # 4-byte Spill
	movaps	%xmm7, %xmm0
	mulss	%xmm7, %xmm0
	mulss	%xmm1, %xmm1
	addss	%xmm0, %xmm1
	movaps	%xmm4, %xmm0
	movss	%xmm2, 192(%rsp)                # 4-byte Spill
	subss	%xmm13, %xmm0
	movss	%xmm0, 200(%rsp)                # 4-byte Spill
	mulss	%xmm0, %xmm0
	addss	%xmm1, %xmm0
	movaps	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	xorps	%xmm0, %xmm0
	rsqrtss	%xmm1, %xmm0
	mulss	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	addss	%xmm5, %xmm1
	mulss	%xmm8, %xmm0
	mulss	%xmm15, %xmm0
	mulss	%xmm1, %xmm0
	movss	%xmm0, 168(%rsp)                # 4-byte Spill
	movaps	%xmm11, %xmm0
	movss	%xmm13, 224(%rsp)               # 4-byte Spill
	subss	80(%rsp), %xmm0                 # 4-byte Folded Reload
	movss	%xmm0, 232(%rsp)                # 4-byte Spill
	movaps	%xmm6, %xmm1
	movaps	%xmm6, %xmm7
	movss	%xmm6, 412(%rsp)                # 4-byte Spill
	movss	84(%rsp), %xmm14                # 4-byte Reload
                                        # xmm14 = mem[0],zero,zero,zero
	subss	%xmm14, %xmm1
	movss	%xmm1, 236(%rsp)                # 4-byte Spill
	mulss	%xmm0, %xmm0
	mulss	%xmm1, %xmm1
	addss	%xmm0, %xmm1
	movaps	%xmm4, %xmm0
	movaps	%xmm4, %xmm2
	movss	%xmm4, 404(%rsp)                # 4-byte Spill
	movaps	%xmm11, %xmm6
	movss	%xmm11, 408(%rsp)               # 4-byte Spill
	movss	88(%rsp), %xmm11                # 4-byte Reload
                                        # xmm11 = mem[0],zero,zero,zero
	subss	%xmm11, %xmm0
	movss	%xmm0, 60(%rsp)                 # 4-byte Spill
	mulss	%xmm0, %xmm0
	addss	%xmm1, %xmm0
	movaps	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	xorps	%xmm0, %xmm0
	rsqrtss	%xmm1, %xmm0
	mulss	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	addss	%xmm5, %xmm1
	mulss	%xmm8, %xmm0
	mulss	%xmm15, %xmm0
	mulss	%xmm1, %xmm0
	movss	%xmm0, 164(%rsp)                # 4-byte Spill
	movaps	%xmm6, %xmm0
	subss	92(%rsp), %xmm0                 # 4-byte Folded Reload
	movss	%xmm0, 56(%rsp)                 # 4-byte Spill
	movaps	%xmm7, %xmm1
	movss	72(%rsp), %xmm10                # 4-byte Reload
                                        # xmm10 = mem[0],zero,zero,zero
	subss	%xmm10, %xmm1
	movss	%xmm1, 64(%rsp)                 # 4-byte Spill
	mulss	%xmm0, %xmm0
	mulss	%xmm1, %xmm1
	addss	%xmm0, %xmm1
	movaps	%xmm4, %xmm0
	movss	96(%rsp), %xmm6                 # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	subss	%xmm6, %xmm0
	movss	%xmm0, 68(%rsp)                 # 4-byte Spill
	mulss	%xmm0, %xmm0
	addss	%xmm1, %xmm0
	movaps	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	xorps	%xmm0, %xmm0
	rsqrtss	%xmm1, %xmm0
	mulss	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	addss	%xmm5, %xmm1
	movaps	%xmm8, %xmm7
	mulss	%xmm8, %xmm0
	mulss	%xmm15, %xmm0
	mulss	%xmm1, %xmm0
	movss	%xmm0, 160(%rsp)                # 4-byte Spill
	movaps	%xmm3, %xmm2
	subss	%xmm12, %xmm2
	movss	%xmm2, 8(%rsp)                  # 4-byte Spill
	movss	16(%rsp), %xmm4                 # 4-byte Reload
                                        # xmm4 = mem[0],zero,zero,zero
	movaps	%xmm4, %xmm0
	movaps	%xmm4, %xmm1
	subss	%xmm9, %xmm1
	movss	%xmm1, (%rsp)                   # 4-byte Spill
	movaps	%xmm2, %xmm0
	mulss	%xmm2, %xmm0
	mulss	%xmm1, %xmm1
	addss	%xmm0, %xmm1
	movss	20(%rsp), %xmm8                 # 4-byte Reload
                                        # xmm8 = mem[0],zero,zero,zero
	movaps	%xmm8, %xmm0
	subss	%xmm13, %xmm0
	movss	%xmm0, 112(%rsp)                # 4-byte Spill
	mulss	%xmm0, %xmm0
	addss	%xmm1, %xmm0
	movaps	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	rsqrtss	%xmm1, %xmm12
	mulss	%xmm12, %xmm1
	mulss	%xmm12, %xmm1
	addss	%xmm5, %xmm1
	mulss	%xmm7, %xmm12
	movaps	%xmm7, %xmm9
	mulss	%xmm15, %xmm12
	mulss	%xmm1, %xmm12
	movaps	%xmm3, %xmm0
	movss	%xmm3, 400(%rsp)                # 4-byte Spill
	movss	80(%rsp), %xmm2                 # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	subss	%xmm2, %xmm0
	movss	%xmm0, 180(%rsp)                # 4-byte Spill
	movaps	%xmm4, %xmm1
	subss	%xmm14, %xmm1
	movss	%xmm1, 196(%rsp)                # 4-byte Spill
	mulss	%xmm0, %xmm0
	mulss	%xmm1, %xmm1
	addss	%xmm0, %xmm1
	movaps	%xmm8, %xmm0
	subss	%xmm11, %xmm0
	movss	%xmm0, 184(%rsp)                # 4-byte Spill
	movaps	%xmm11, %xmm7
	mulss	%xmm0, %xmm0
	addss	%xmm1, %xmm0
	movaps	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	xorps	%xmm0, %xmm0
	rsqrtss	%xmm1, %xmm0
	mulss	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	addss	%xmm5, %xmm1
	movaps	%xmm5, %xmm13
	mulss	%xmm9, %xmm0
	mulss	%xmm15, %xmm0
	mulss	%xmm1, %xmm0
	movss	%xmm0, 392(%rsp)                # 4-byte Spill
	movaps	%xmm3, %xmm1
	movss	92(%rsp), %xmm5                 # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	subss	%xmm5, %xmm1
	movss	%xmm1, 204(%rsp)                # 4-byte Spill
	movaps	%xmm4, %xmm3
	subss	%xmm10, %xmm3
	movss	%xmm3, 208(%rsp)                # 4-byte Spill
	mulss	%xmm1, %xmm1
	movaps	%xmm10, %xmm4
	mulss	%xmm3, %xmm3
	addss	%xmm1, %xmm3
	subss	%xmm6, %xmm8
	movss	%xmm8, 220(%rsp)                # 4-byte Spill
	mulss	%xmm8, %xmm8
	addss	%xmm3, %xmm8
	movaps	%xmm8, %xmm3
	mulss	%xmm8, %xmm3
	mulss	%xmm8, %xmm3
	xorps	%xmm0, %xmm0
	rsqrtss	%xmm3, %xmm0
	mulss	%xmm0, %xmm3
	mulss	%xmm0, %xmm3
	addss	%xmm13, %xmm3
	movaps	%xmm13, %xmm11
	mulss	%xmm9, %xmm0
	mulss	%xmm15, %xmm0
	mulss	%xmm3, %xmm0
	movss	%xmm0, 156(%rsp)                # 4-byte Spill
	movss	12(%rsp), %xmm13                # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	movaps	%xmm13, %xmm1
	subss	%xmm2, %xmm1
	movss	%xmm1, 24(%rsp)                 # 4-byte Spill
	movss	228(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	movaps	%xmm2, %xmm3
	subss	%xmm14, %xmm3
	movss	%xmm3, 4(%rsp)                  # 4-byte Spill
	mulss	%xmm1, %xmm1
	mulss	%xmm3, %xmm3
	addss	%xmm1, %xmm3
	movss	224(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movaps	%xmm0, %xmm1
	movaps	%xmm7, %xmm8
	movss	%xmm7, 88(%rsp)                 # 4-byte Spill
	subss	%xmm7, %xmm1
	movss	%xmm1, 104(%rsp)                # 4-byte Spill
	mulss	%xmm1, %xmm1
	addss	%xmm3, %xmm1
	movaps	%xmm1, %xmm3
	mulss	%xmm1, %xmm3
	mulss	%xmm1, %xmm3
	rsqrtss	%xmm3, %xmm10
	mulss	%xmm10, %xmm3
	mulss	%xmm10, %xmm3
	addss	%xmm11, %xmm3
	mulss	%xmm9, %xmm10
	mulss	%xmm15, %xmm10
	movaps	%xmm15, %xmm1
	mulss	%xmm3, %xmm10
	subss	%xmm5, %xmm13
	movss	%xmm13, 152(%rsp)               # 4-byte Spill
	movaps	%xmm2, %xmm7
	movaps	%xmm4, %xmm2
	movss	%xmm4, 72(%rsp)                 # 4-byte Spill
	subss	%xmm4, %xmm7
	movss	%xmm7, 172(%rsp)                # 4-byte Spill
	mulss	%xmm13, %xmm13
	movss	%xmm14, 84(%rsp)                # 4-byte Spill
	movaps	%xmm7, %xmm4
	mulss	%xmm7, %xmm4
	addss	%xmm13, %xmm4
	subss	%xmm6, %xmm0
	movss	%xmm0, 188(%rsp)                # 4-byte Spill
	mulss	%xmm0, %xmm0
	addss	%xmm4, %xmm0
	movaps	%xmm0, %xmm4
	mulss	%xmm0, %xmm4
	mulss	%xmm0, %xmm4
	movaps	%xmm5, %xmm0
	xorps	%xmm11, %xmm11
	rsqrtss	%xmm4, %xmm11
	mulss	%xmm11, %xmm4
	mulss	%xmm11, %xmm4
	movss	.LCPI6_15(%rip), %xmm7          # xmm7 = [-3.0E+0,0.0E+0,0.0E+0,0.0E+0]
	addss	%xmm7, %xmm4
	movss	.LCPI6_16(%rip), %xmm5          # xmm5 = [-5.0E-1,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm5, %xmm11
	mulss	%xmm15, %xmm11
	movss	%xmm15, 32(%rsp)                # 4-byte Spill
	mulss	%xmm4, %xmm11
	movss	80(%rsp), %xmm3                 # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	subss	%xmm0, %xmm3
	movss	%xmm3, 28(%rsp)                 # 4-byte Spill
	subss	%xmm2, %xmm14
	movss	%xmm14, 100(%rsp)               # 4-byte Spill
	mulss	%xmm3, %xmm3
	mulss	%xmm14, %xmm14
	addss	%xmm3, %xmm14
	movaps	%xmm8, %xmm3
	subss	%xmm6, %xmm3
	movss	%xmm3, 108(%rsp)                # 4-byte Spill
	mulss	%xmm3, %xmm3
	addss	%xmm14, %xmm3
	movaps	%xmm3, %xmm4
	mulss	%xmm3, %xmm4
	mulss	%xmm3, %xmm4
	xorps	%xmm13, %xmm13
	rsqrtss	%xmm4, %xmm13
	mulss	%xmm13, %xmm4
	mulss	%xmm13, %xmm4
	addss	%xmm7, %xmm4
	mulss	%xmm5, %xmm13
	mulss	%xmm15, %xmm13
	mulss	%xmm4, %xmm13
	movss	(%rsi), %xmm15                  # xmm15 = mem[0],zero,zero,zero
	movss	176(%rsp), %xmm3                # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm3
	movss	396(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm6, %xmm3
	movss	40(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	subss	%xmm3, %xmm0
	movss	(%rdx), %xmm14                  # xmm14 = mem[0],zero,zero,zero
	movss	212(%rsp), %xmm3                # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm3
	movss	168(%rsp), %xmm4                # 4-byte Reload
                                        # xmm4 = mem[0],zero,zero,zero
	mulss	%xmm4, %xmm3
	subss	%xmm3, %xmm0
	movss	(%rcx), %xmm8                   # xmm8 = mem[0],zero,zero,zero
	movss	232(%rsp), %xmm3                # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm3
	movss	388(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	movss	164(%rsp), %xmm5                # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm3
	subss	%xmm3, %xmm0
	movss	(%rax), %xmm7                   # xmm7 = mem[0],zero,zero,zero
	movss	56(%rsp), %xmm3                 # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm3
	movss	160(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm2, %xmm3
	subss	%xmm3, %xmm0
	movss	%xmm0, 40(%rsp)                 # 4-byte Spill
	movaps	%xmm1, %xmm3
	mulss	%xmm15, %xmm3
	mulss	%xmm6, %xmm3
	movss	36(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	subss	%xmm3, %xmm0
	movss	216(%rsp), %xmm3                # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm3
	mulss	%xmm4, %xmm3
	subss	%xmm3, %xmm0
	movss	236(%rsp), %xmm3                # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm3
	mulss	%xmm5, %xmm3
	movaps	%xmm5, %xmm9
	subss	%xmm3, %xmm0
	movss	64(%rsp), %xmm3                 # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm3
	mulss	%xmm2, %xmm3
	subss	%xmm3, %xmm0
	movss	%xmm0, 36(%rsp)                 # 4-byte Spill
	movss	192(%rsp), %xmm3                # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm3
	mulss	%xmm6, %xmm3
	movss	44(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	subss	%xmm3, %xmm0
	movss	200(%rsp), %xmm3                # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm3
	mulss	%xmm4, %xmm3
	subss	%xmm3, %xmm0
	movss	60(%rsp), %xmm3                 # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm3
	mulss	%xmm5, %xmm3
	subss	%xmm3, %xmm0
	movss	68(%rsp), %xmm3                 # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm3
	mulss	%xmm2, %xmm3
	subss	%xmm3, %xmm0
	movss	%xmm0, 44(%rsp)                 # 4-byte Spill
	movss	(%rdi), %xmm3                   # xmm3 = mem[0],zero,zero,zero
	movss	176(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm2
	mulss	%xmm6, %xmm2
	movaps	%xmm2, %xmm9
	movaps	%xmm6, %xmm2
	movss	52(%rsp), %xmm5                 # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	addss	%xmm9, %xmm5
	movss	8(%rsp), %xmm9                  # 4-byte Reload
                                        # xmm9 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm9
	mulss	%xmm12, %xmm9
	subss	%xmm9, %xmm5
	movss	180(%rsp), %xmm9                # 4-byte Reload
                                        # xmm9 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm9
	movss	392(%rsp), %xmm4                # 4-byte Reload
                                        # xmm4 = mem[0],zero,zero,zero
	mulss	%xmm4, %xmm9
	subss	%xmm9, %xmm5
	movss	204(%rsp), %xmm9                # 4-byte Reload
                                        # xmm9 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm9
	movss	156(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm0, %xmm9
	subss	%xmm9, %xmm5
	movss	%xmm5, 52(%rsp)                 # 4-byte Spill
	mulss	%xmm3, %xmm1
	mulss	%xmm6, %xmm1
	movss	48(%rsp), %xmm9                 # 4-byte Reload
                                        # xmm9 = mem[0],zero,zero,zero
	addss	%xmm1, %xmm9
	movss	(%rsp), %xmm5                   # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm5
	mulss	%xmm12, %xmm5
	subss	%xmm5, %xmm9
	movss	196(%rsp), %xmm5                # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm5
	mulss	%xmm4, %xmm5
	subss	%xmm5, %xmm9
	movss	208(%rsp), %xmm5                # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm5
	mulss	%xmm0, %xmm5
	movaps	%xmm0, %xmm6
	subss	%xmm5, %xmm9
	movss	%xmm9, 48(%rsp)                 # 4-byte Spill
	movss	192(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm1
	mulss	%xmm2, %xmm1
	movss	76(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	addss	%xmm1, %xmm0
	movss	112(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	movaps	%xmm1, %xmm2
	mulss	%xmm14, %xmm2
	mulss	%xmm12, %xmm2
	subss	%xmm2, %xmm0
	movss	184(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm2
	mulss	%xmm4, %xmm2
	subss	%xmm2, %xmm0
	movss	220(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm2
	mulss	%xmm6, %xmm2
	subss	%xmm2, %xmm0
	movss	%xmm0, 76(%rsp)                 # 4-byte Spill
	movss	212(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm2
	movss	168(%rsp), %xmm9                # 4-byte Reload
                                        # xmm9 = mem[0],zero,zero,zero
	mulss	%xmm9, %xmm2
	movaps	%xmm2, %xmm5
	movss	116(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	addss	%xmm5, %xmm2
	movss	8(%rsp), %xmm0                  # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm0
	mulss	%xmm12, %xmm0
	addss	%xmm2, %xmm0
	movss	24(%rsp), %xmm2                 # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm2
	mulss	%xmm10, %xmm2
	subss	%xmm2, %xmm0
	movss	152(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm2
	mulss	%xmm11, %xmm2
	subss	%xmm2, %xmm0
	movss	%xmm0, 8(%rsp)                  # 4-byte Spill
	movss	216(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm2
	mulss	%xmm9, %xmm2
	movaps	%xmm2, %xmm5
	movss	120(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	addss	%xmm5, %xmm2
	movss	(%rsp), %xmm0                   # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm0
	mulss	%xmm12, %xmm0
	addss	%xmm2, %xmm0
	movss	4(%rsp), %xmm2                  # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm2
	mulss	%xmm10, %xmm2
	subss	%xmm2, %xmm0
	movss	172(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm2
	mulss	%xmm11, %xmm2
	subss	%xmm2, %xmm0
	movss	%xmm0, (%rsp)                   # 4-byte Spill
	movss	200(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm0
	mulss	%xmm9, %xmm0
	movss	124(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	addss	%xmm0, %xmm2
	mulss	%xmm15, %xmm1
	mulss	%xmm12, %xmm1
	addss	%xmm2, %xmm1
	movss	104(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	movaps	%xmm6, %xmm2
	mulss	%xmm8, %xmm2
	mulss	%xmm10, %xmm2
	subss	%xmm2, %xmm1
	movss	188(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm2
	mulss	%xmm11, %xmm2
	subss	%xmm2, %xmm1
	movss	%xmm1, 112(%rsp)                # 4-byte Spill
	movss	232(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm0
	movss	164(%rsp), %xmm5                # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm0
	movss	128(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	addss	%xmm0, %xmm2
	movss	180(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm0
	mulss	%xmm4, %xmm0
	movaps	%xmm0, %xmm12
	movss	24(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm0
	mulss	%xmm10, %xmm0
	addss	%xmm12, %xmm0
	addss	%xmm2, %xmm0
	movss	28(%rsp), %xmm2                 # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm7, %xmm2
	mulss	%xmm13, %xmm2
	subss	%xmm2, %xmm0
	movss	%xmm0, 24(%rsp)                 # 4-byte Spill
	movss	236(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm0
	mulss	%xmm5, %xmm0
	movss	132(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	addss	%xmm0, %xmm2
	movss	196(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm0
	mulss	%xmm4, %xmm0
	movaps	%xmm0, %xmm9
	movss	4(%rsp), %xmm0                  # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm0
	mulss	%xmm10, %xmm0
	addss	%xmm9, %xmm0
	addss	%xmm2, %xmm0
	movss	100(%rsp), %xmm9                # 4-byte Reload
                                        # xmm9 = mem[0],zero,zero,zero
	movaps	%xmm9, %xmm2
	mulss	%xmm7, %xmm2
	mulss	%xmm13, %xmm2
	subss	%xmm2, %xmm0
	movss	%xmm0, 4(%rsp)                  # 4-byte Spill
	movss	60(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm0
	mulss	%xmm5, %xmm0
	movss	136(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	addss	%xmm0, %xmm2
	movss	184(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm0
	mulss	%xmm4, %xmm0
	movaps	%xmm0, %xmm1
	mulss	%xmm14, %xmm6
	mulss	%xmm10, %xmm6
	addss	%xmm0, %xmm6
	addss	%xmm2, %xmm6
	movss	108(%rsp), %xmm5                # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm7
	mulss	%xmm13, %xmm7
	subss	%xmm7, %xmm6
	movaps	%xmm6, %xmm12
	movss	%xmm6, 104(%rsp)                # 4-byte Spill
	movss	56(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm0
	movss	160(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm2, %xmm0
	movaps	%xmm0, %xmm4
	movss	140(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	addss	%xmm4, %xmm0
	movss	204(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm1
	movss	156(%rsp), %xmm4                # 4-byte Reload
                                        # xmm4 = mem[0],zero,zero,zero
	mulss	%xmm4, %xmm1
	movss	152(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm6
	mulss	%xmm11, %xmm6
	addss	%xmm1, %xmm6
	addss	%xmm0, %xmm6
	movss	28(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm8, %xmm0
	mulss	%xmm13, %xmm0
	addss	%xmm6, %xmm0
	movss	%xmm0, 28(%rsp)                 # 4-byte Spill
	movss	64(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm0
	mulss	%xmm2, %xmm0
	movaps	%xmm0, %xmm1
	movss	144(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	addss	%xmm1, %xmm0
	movss	208(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm1
	mulss	%xmm4, %xmm1
	movss	172(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm6
	mulss	%xmm11, %xmm6
	addss	%xmm1, %xmm6
	addss	%xmm0, %xmm6
	movaps	%xmm9, %xmm0
	mulss	%xmm8, %xmm0
	mulss	%xmm13, %xmm0
	addss	%xmm6, %xmm0
	movss	%xmm0, 100(%rsp)                # 4-byte Spill
	movss	68(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm0
	mulss	%xmm2, %xmm0
	movaps	%xmm0, %xmm1
	movss	148(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	addss	%xmm1, %xmm0
	movss	220(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm1
	mulss	%xmm4, %xmm1
	movss	188(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm14, %xmm2
	mulss	%xmm11, %xmm2
	addss	%xmm1, %xmm2
	addss	%xmm0, %xmm2
	movaps	%xmm5, %xmm0
	mulss	%xmm8, %xmm0
	mulss	%xmm13, %xmm0
	addss	%xmm2, %xmm0
	movss	%xmm0, 108(%rsp)                # 4-byte Spill
	movss	32(%rsp), %xmm14                # 4-byte Reload
                                        # xmm14 = mem[0],zero,zero,zero
	movaps	%xmm14, %xmm0
	mulss	40(%rsp), %xmm0                 # 4-byte Folded Reload
	addss	408(%rsp), %xmm0                # 4-byte Folded Reload
	movss	%xmm0, 68(%rsp)                 # 4-byte Spill
	movaps	%xmm14, %xmm0
	movss	36(%rsp), %xmm1                 # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm1, %xmm0
	addss	412(%rsp), %xmm0                # 4-byte Folded Reload
	movss	%xmm0, 64(%rsp)                 # 4-byte Spill
	movaps	%xmm14, %xmm0
	movss	44(%rsp), %xmm1                 # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm1, %xmm0
	addss	404(%rsp), %xmm0                # 4-byte Folded Reload
	movss	%xmm0, 60(%rsp)                 # 4-byte Spill
	movaps	%xmm14, %xmm1
	movss	52(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm0, %xmm1
	addss	400(%rsp), %xmm1                # 4-byte Folded Reload
	movss	%xmm1, 56(%rsp)                 # 4-byte Spill
	movaps	%xmm14, %xmm1
	movss	48(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm0, %xmm1
	addss	16(%rsp), %xmm1                 # 4-byte Folded Reload
	movss	%xmm1, 16(%rsp)                 # 4-byte Spill
	movaps	%xmm14, %xmm0
	movss	76(%rsp), %xmm6                 # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm6, %xmm0
	addss	20(%rsp), %xmm0                 # 4-byte Folded Reload
	movss	%xmm0, 20(%rsp)                 # 4-byte Spill
	movaps	%xmm14, %xmm0
	mulss	8(%rsp), %xmm0                  # 4-byte Folded Reload
	addss	12(%rsp), %xmm0                 # 4-byte Folded Reload
	movss	%xmm0, 12(%rsp)                 # 4-byte Spill
	movaps	%xmm14, %xmm7
	mulss	(%rsp), %xmm7                   # 4-byte Folded Reload
	addss	228(%rsp), %xmm7                # 4-byte Folded Reload
	movaps	%xmm14, %xmm8
	movss	112(%rsp), %xmm5                # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm8
	addss	224(%rsp), %xmm8                # 4-byte Folded Reload
	movaps	%xmm14, %xmm9
	movss	24(%rsp), %xmm3                 # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm9
	addss	80(%rsp), %xmm9                 # 4-byte Folded Reload
	movaps	%xmm14, %xmm10
	movss	4(%rsp), %xmm15                 # 4-byte Reload
                                        # xmm15 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm10
	addss	84(%rsp), %xmm10                # 4-byte Folded Reload
	movaps	%xmm14, %xmm11
	mulss	%xmm12, %xmm11
	addss	88(%rsp), %xmm11                # 4-byte Folded Reload
	movaps	%xmm14, %xmm12
	movss	28(%rsp), %xmm4                 # 4-byte Reload
                                        # xmm4 = mem[0],zero,zero,zero
	mulss	%xmm4, %xmm12
	addss	92(%rsp), %xmm12                # 4-byte Folded Reload
	movaps	%xmm14, %xmm13
	movss	100(%rsp), %xmm2                # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm2, %xmm13
	addss	72(%rsp), %xmm13                # 4-byte Folded Reload
	movss	108(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm1, %xmm14
	addss	96(%rsp), %xmm14                # 4-byte Folded Reload
	movss	%xmm14, 32(%rsp)                # 4-byte Spill
	movss	40(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 316(%rsp)
	movss	36(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 336(%rsp)
	movss	44(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 356(%rsp)
	movss	52(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 320(%rsp)
	movss	48(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 340(%rsp)
	movss	%xmm6, 360(%rsp)
	movss	8(%rsp), %xmm6                  # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	movss	%xmm6, 324(%rsp)
	movss	(%rsp), %xmm0                   # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 344(%rsp)
	movss	%xmm5, 364(%rsp)
	movss	%xmm3, 328(%rsp)
	movss	%xmm15, 348(%rsp)
	movss	104(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 368(%rsp)
	movss	%xmm4, 332(%rsp)
	movss	%xmm2, 352(%rsp)
	movss	%xmm1, 372(%rsp)
	movss	68(%rsp), %xmm2                 # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	movss	%xmm2, 256(%rsp)
	movss	64(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 276(%rsp)
	movss	60(%rsp), %xmm4                 # 4-byte Reload
                                        # xmm4 = mem[0],zero,zero,zero
	movss	%xmm4, 296(%rsp)
	movss	56(%rsp), %xmm15                # 4-byte Reload
                                        # xmm15 = mem[0],zero,zero,zero
	movss	%xmm15, 260(%rsp)
	movss	16(%rsp), %xmm1                 # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	movss	%xmm1, 280(%rsp)
	movss	20(%rsp), %xmm3                 # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	movss	%xmm3, 300(%rsp)
	movss	12(%rsp), %xmm5                 # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	movss	%xmm5, 264(%rsp)
	movss	%xmm7, 284(%rsp)
	movss	%xmm8, 304(%rsp)
	movss	%xmm9, 268(%rsp)
	movss	%xmm10, 288(%rsp)
	movss	%xmm11, 308(%rsp)
	movss	%xmm12, 272(%rsp)
	movss	%xmm13, 292(%rsp)
	movss	%xmm14, 312(%rsp)
	incq	%rbx
	movq	%rbx, 248(%rsp)
	xorps	%xmm14, %xmm14
	addss	%xmm14, %xmm2
	movss	%xmm2, 68(%rsp)                 # 4-byte Spill
	addss	%xmm14, %xmm15
	movss	%xmm15, 56(%rsp)                # 4-byte Spill
	addss	%xmm14, %xmm5
	movss	%xmm5, 12(%rsp)                 # 4-byte Spill
	addss	%xmm14, %xmm9
	addss	%xmm14, %xmm12
	addss	%xmm14, %xmm0
	movss	%xmm0, 64(%rsp)                 # 4-byte Spill
	addss	%xmm14, %xmm1
	movss	%xmm1, 16(%rsp)                 # 4-byte Spill
	addss	%xmm14, %xmm7
	addss	%xmm14, %xmm10
	addss	%xmm14, %xmm13
	addss	%xmm14, %xmm4
	movss	%xmm4, 60(%rsp)                 # 4-byte Spill
	addss	%xmm14, %xmm3
	movss	%xmm3, 20(%rsp)                 # 4-byte Spill
	addss	%xmm14, %xmm8
	addss	%xmm14, %xmm11
	movss	32(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm0
	movss	%xmm0, 32(%rsp)                 # 4-byte Spill
	movss	40(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm0
	movss	%xmm0, 40(%rsp)                 # 4-byte Spill
	movss	52(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm0
	movss	%xmm0, 52(%rsp)                 # 4-byte Spill
	addss	%xmm14, %xmm6
	movss	%xmm6, 8(%rsp)                  # 4-byte Spill
	movss	24(%rsp), %xmm2                 # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm2
	movss	28(%rsp), %xmm1                 # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm1
	movss	36(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm0
	movss	%xmm0, 36(%rsp)                 # 4-byte Spill
	movss	48(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm0
	movss	%xmm0, 48(%rsp)                 # 4-byte Spill
	movss	(%rsp), %xmm0                   # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm0
	movss	%xmm0, (%rsp)                   # 4-byte Spill
	movss	4(%rsp), %xmm0                  # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm0
	movss	%xmm0, 4(%rsp)                  # 4-byte Spill
	movss	100(%rsp), %xmm4                # 4-byte Reload
                                        # xmm4 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm4
	movss	44(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm0
	movss	%xmm0, 44(%rsp)                 # 4-byte Spill
	movss	76(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm0
	movss	%xmm0, 76(%rsp)                 # 4-byte Spill
	movss	112(%rsp), %xmm15               # 4-byte Reload
                                        # xmm15 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm15
	movss	104(%rsp), %xmm3                # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm3
	movss	108(%rsp), %xmm5                # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	addss	%xmm14, %xmm5
	movss	68(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	56(%rsp), %xmm14                # 4-byte Reload
                                        # xmm14 = mem[0],zero,zero,zero
	movss	%xmm9, 80(%rsp)                 # 4-byte Spill
	movss	%xmm12, 92(%rsp)                # 4-byte Spill
	movss	64(%rsp), %xmm12                # 4-byte Reload
                                        # xmm12 = mem[0],zero,zero,zero
	movss	16(%rsp), %xmm9                 # 4-byte Reload
                                        # xmm9 = mem[0],zero,zero,zero
	movss	%xmm10, 84(%rsp)                # 4-byte Spill
	movss	%xmm13, 72(%rsp)                # 4-byte Spill
	movss	60(%rsp), %xmm10                # 4-byte Reload
                                        # xmm10 = mem[0],zero,zero,zero
	movaps	%xmm8, %xmm13
	movss	%xmm11, 88(%rsp)                # 4-byte Spill
	movaps	%xmm0, %xmm11
	movss	32(%rsp), %xmm0                 # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 96(%rsp)                 # 4-byte Spill
	movss	8(%rsp), %xmm0                  # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 116(%rsp)                # 4-byte Spill
	movss	%xmm2, 128(%rsp)                # 4-byte Spill
	movss	%xmm1, 140(%rsp)                # 4-byte Spill
	movss	(%rsp), %xmm0                   # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 120(%rsp)                # 4-byte Spill
	movss	4(%rsp), %xmm0                  # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	movss	%xmm0, 132(%rsp)                # 4-byte Spill
	movss	%xmm4, 144(%rsp)                # 4-byte Spill
	movss	%xmm15, 124(%rsp)               # 4-byte Spill
	movss	%xmm3, 136(%rsp)                # 4-byte Spill
	movss	%xmm5, 148(%rsp)                # 4-byte Spill
	jmp	.LBB6_1
.LBB6_3:                                # %.cm_end_221
	mulss	%xmm0, %xmm0
	mulss	%xmm12, %xmm12
	addss	%xmm0, %xmm12
	mulss	%xmm2, %xmm2
	addss	%xmm12, %xmm2
	xorps	%xmm0, %xmm0
	rsqrtss	%xmm2, %xmm0
	mulss	%xmm0, %xmm2
	mulss	%xmm0, %xmm2
	movaps	%xmm11, %xmm5
	movss	.LCPI6_15(%rip), %xmm11         # xmm11 = [-3.0E+0,0.0E+0,0.0E+0,0.0E+0]
	addss	%xmm11, %xmm2
	movaps	%xmm11, %xmm3
	movss	20(%rsp), %xmm9                 # 4-byte Reload
                                        # xmm9 = mem[0],zero,zero,zero
	movss	.LCPI6_16(%rip), %xmm10         # xmm10 = [-5.0E-1,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm10, %xmm0
	movaps	%xmm10, %xmm12
	mulss	%xmm2, %xmm0
	movss	%xmm0, (%rsp)                   # 4-byte Spill
	movaps	%xmm5, %xmm1
	movss	12(%rsp), %xmm11                # 4-byte Reload
                                        # xmm11 = mem[0],zero,zero,zero
	subss	%xmm11, %xmm1
	mulss	%xmm1, %xmm1
	movaps	%xmm6, %xmm8
	movaps	%xmm6, %xmm2
	subss	%xmm7, %xmm2
	mulss	%xmm2, %xmm2
	addss	%xmm1, %xmm2
	movaps	%xmm4, %xmm6
	movaps	%xmm4, %xmm1
	subss	%xmm13, %xmm1
	mulss	%xmm1, %xmm1
	addss	%xmm2, %xmm1
	xorps	%xmm0, %xmm0
	rsqrtss	%xmm1, %xmm0
	mulss	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	movaps	%xmm3, %xmm10
	addss	%xmm3, %xmm1
	mulss	%xmm12, %xmm0
	mulss	%xmm1, %xmm0
	movss	%xmm0, 4(%rsp)                  # 4-byte Spill
	movaps	%xmm5, %xmm1
	movaps	%xmm13, %xmm3
	movss	80(%rsp), %xmm15                # 4-byte Reload
                                        # xmm15 = mem[0],zero,zero,zero
	subss	%xmm15, %xmm1
	mulss	%xmm1, %xmm1
	movaps	%xmm8, %xmm0
	movss	84(%rsp), %xmm4                 # 4-byte Reload
                                        # xmm4 = mem[0],zero,zero,zero
	subss	%xmm4, %xmm0
	mulss	%xmm0, %xmm0
	addss	%xmm1, %xmm0
	movaps	%xmm6, %xmm1
	movss	88(%rsp), %xmm2                 # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	subss	%xmm2, %xmm1
	mulss	%xmm1, %xmm1
	addss	%xmm0, %xmm1
	xorps	%xmm0, %xmm0
	rsqrtss	%xmm1, %xmm0
	mulss	%xmm0, %xmm1
	mulss	%xmm0, %xmm1
	addss	%xmm10, %xmm1
	mulss	%xmm12, %xmm0
	mulss	%xmm1, %xmm0
	movss	%xmm0, 28(%rsp)                 # 4-byte Spill
	movss	92(%rsp), %xmm10                # 4-byte Reload
                                        # xmm10 = mem[0],zero,zero,zero
	subss	%xmm10, %xmm5
	mulss	%xmm5, %xmm5
	subss	72(%rsp), %xmm8                 # 4-byte Folded Reload
	mulss	%xmm8, %xmm8
	addss	%xmm5, %xmm8
	movss	96(%rsp), %xmm13                # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	subss	%xmm13, %xmm6
	mulss	%xmm6, %xmm6
	addss	%xmm8, %xmm6
	xorps	%xmm0, %xmm0
	rsqrtss	%xmm6, %xmm0
	mulss	%xmm0, %xmm6
	mulss	%xmm0, %xmm6
	movss	.LCPI6_15(%rip), %xmm8          # xmm8 = [-3.0E+0,0.0E+0,0.0E+0,0.0E+0]
	addss	%xmm8, %xmm6
	mulss	%xmm12, %xmm0
	mulss	%xmm6, %xmm0
	movss	%xmm0, 8(%rsp)                  # 4-byte Spill
	movaps	%xmm14, %xmm0
	subss	%xmm11, %xmm0
	mulss	%xmm0, %xmm0
	movss	16(%rsp), %xmm1                 # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	movaps	%xmm1, %xmm5
	subss	%xmm7, %xmm5
	mulss	%xmm5, %xmm5
	addss	%xmm0, %xmm5
	movaps	%xmm9, %xmm6
	subss	%xmm3, %xmm6
	mulss	%xmm6, %xmm6
	addss	%xmm5, %xmm6
	xorps	%xmm0, %xmm0
	rsqrtss	%xmm6, %xmm0
	mulss	%xmm0, %xmm6
	mulss	%xmm0, %xmm6
	addss	%xmm8, %xmm6
	mulss	%xmm12, %xmm0
	mulss	%xmm6, %xmm0
	movss	%xmm0, 32(%rsp)                 # 4-byte Spill
	movaps	%xmm14, %xmm5
	subss	%xmm15, %xmm5
	mulss	%xmm5, %xmm5
	movaps	%xmm1, %xmm6
	subss	%xmm4, %xmm6
	mulss	%xmm6, %xmm6
	addss	%xmm5, %xmm6
	movaps	%xmm14, %xmm8
	movaps	%xmm7, %xmm0
	movaps	%xmm9, %xmm7
	subss	%xmm2, %xmm7
	mulss	%xmm7, %xmm7
	addss	%xmm6, %xmm7
	xorps	%xmm5, %xmm5
	rsqrtss	%xmm7, %xmm5
	mulss	%xmm5, %xmm7
	mulss	%xmm5, %xmm7
	movss	.LCPI6_15(%rip), %xmm14         # xmm14 = [-3.0E+0,0.0E+0,0.0E+0,0.0E+0]
	addss	%xmm14, %xmm7
	mulss	%xmm12, %xmm5
	mulss	%xmm7, %xmm5
	movss	%xmm5, 24(%rsp)                 # 4-byte Spill
	subss	%xmm10, %xmm8
	mulss	%xmm8, %xmm8
	movss	72(%rsp), %xmm5                 # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	subss	%xmm5, %xmm1
	mulss	%xmm1, %xmm1
	addss	%xmm8, %xmm1
	subss	%xmm13, %xmm9
	mulss	%xmm9, %xmm9
	addss	%xmm1, %xmm9
	xorps	%xmm6, %xmm6
	rsqrtss	%xmm9, %xmm6
	mulss	%xmm6, %xmm9
	mulss	%xmm6, %xmm9
	addss	%xmm14, %xmm9
	mulss	%xmm12, %xmm6
	mulss	%xmm9, %xmm6
	movaps	%xmm11, %xmm7
	subss	%xmm15, %xmm7
	mulss	%xmm7, %xmm7
	movaps	%xmm0, %xmm8
	subss	%xmm4, %xmm8
	mulss	%xmm8, %xmm8
	addss	%xmm7, %xmm8
	movaps	%xmm3, %xmm9
	subss	%xmm2, %xmm9
	mulss	%xmm9, %xmm9
	addss	%xmm8, %xmm9
	xorps	%xmm7, %xmm7
	rsqrtss	%xmm9, %xmm7
	mulss	%xmm7, %xmm9
	mulss	%xmm7, %xmm9
	addss	%xmm14, %xmm9
	mulss	%xmm12, %xmm7
	mulss	%xmm9, %xmm7
	subss	%xmm10, %xmm11
	mulss	%xmm11, %xmm11
	subss	%xmm5, %xmm0
	mulss	%xmm0, %xmm0
	addss	%xmm11, %xmm0
	subss	%xmm13, %xmm3
	mulss	%xmm3, %xmm3
	addss	%xmm0, %xmm3
	xorps	%xmm8, %xmm8
	rsqrtss	%xmm3, %xmm8
	mulss	%xmm8, %xmm3
	mulss	%xmm8, %xmm3
	addss	%xmm14, %xmm3
	mulss	%xmm12, %xmm8
	mulss	%xmm3, %xmm8
	subss	%xmm10, %xmm15
	subss	%xmm5, %xmm4
	mulss	%xmm15, %xmm15
	mulss	%xmm4, %xmm4
	addss	%xmm15, %xmm4
	subss	%xmm13, %xmm2
	mulss	%xmm2, %xmm2
	addss	%xmm4, %xmm2
	xorps	%xmm9, %xmm9
	rsqrtss	%xmm2, %xmm9
	mulss	%xmm9, %xmm2
	mulss	%xmm9, %xmm2
	addss	%xmm14, %xmm2
	mulss	%xmm12, %xmm9
	mulss	%xmm2, %xmm9
	movss	(%rdi), %xmm12                  # xmm12 = mem[0],zero,zero,zero
	movss	(%rsi), %xmm11                  # xmm11 = mem[0],zero,zero,zero
	movaps	%xmm12, %xmm14
	mulss	%xmm11, %xmm14
	mulss	(%rsp), %xmm14                  # 4-byte Folded Reload
	movaps	%xmm12, %xmm13
	movss	(%rdx), %xmm10                  # xmm10 = mem[0],zero,zero,zero
	mulss	%xmm10, %xmm13
	mulss	4(%rsp), %xmm13                 # 4-byte Folded Reload
	addss	%xmm14, %xmm13
	movaps	%xmm12, %xmm14
	movss	(%rcx), %xmm0                   # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm0, %xmm14
	mulss	28(%rsp), %xmm14                # 4-byte Folded Reload
	movss	40(%rsp), %xmm2                 # 4-byte Reload
                                        # xmm2 = mem[0],zero,zero,zero
	mulss	%xmm2, %xmm2
	movss	36(%rsp), %xmm4                 # 4-byte Reload
                                        # xmm4 = mem[0],zero,zero,zero
	mulss	%xmm4, %xmm4
	addss	%xmm2, %xmm4
	movss	44(%rsp), %xmm15                # 4-byte Reload
                                        # xmm15 = mem[0],zero,zero,zero
	mulss	%xmm15, %xmm15
	addss	%xmm4, %xmm15
	movss	.LCPI6_17(%rip), %xmm2          # xmm2 = [5.0E-1,0.0E+0,0.0E+0,0.0E+0]
	mulss	%xmm2, %xmm15
	mulss	%xmm12, %xmm15
	movss	(%rax), %xmm4                   # xmm4 = mem[0],zero,zero,zero
	mulss	%xmm4, %xmm12
	mulss	8(%rsp), %xmm12                 # 4-byte Folded Reload
	addss	%xmm14, %xmm12
	addss	%xmm13, %xmm12
	movaps	%xmm11, %xmm1
	mulss	%xmm10, %xmm1
	mulss	32(%rsp), %xmm1                 # 4-byte Folded Reload
	movaps	%xmm11, %xmm3
	mulss	%xmm0, %xmm3
	mulss	24(%rsp), %xmm3                 # 4-byte Folded Reload
	addss	%xmm1, %xmm3
	movss	52(%rsp), %xmm1                 # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm1, %xmm1
	movss	48(%rsp), %xmm13                # 4-byte Reload
                                        # xmm13 = mem[0],zero,zero,zero
	mulss	%xmm13, %xmm13
	addss	%xmm1, %xmm13
	movss	76(%rsp), %xmm5                 # 4-byte Reload
                                        # xmm5 = mem[0],zero,zero,zero
	mulss	%xmm5, %xmm5
	addss	%xmm13, %xmm5
	mulss	%xmm2, %xmm5
	mulss	%xmm11, %xmm5
	mulss	%xmm4, %xmm11
	mulss	%xmm6, %xmm11
	addss	%xmm3, %xmm11
	addss	%xmm12, %xmm11
	movaps	%xmm10, %xmm1
	mulss	%xmm0, %xmm1
	mulss	%xmm7, %xmm1
	movss	116(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm6, %xmm6
	movss	120(%rsp), %xmm3                # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm3
	addss	%xmm6, %xmm3
	movss	124(%rsp), %xmm6                # 4-byte Reload
                                        # xmm6 = mem[0],zero,zero,zero
	mulss	%xmm6, %xmm6
	addss	%xmm3, %xmm6
	mulss	%xmm2, %xmm6
	mulss	%xmm10, %xmm6
	mulss	%xmm4, %xmm10
	mulss	%xmm8, %xmm10
	addss	%xmm1, %xmm10
	movss	128(%rsp), %xmm3                # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm3
	movss	132(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm1, %xmm1
	addss	%xmm3, %xmm1
	movss	136(%rsp), %xmm3                # 4-byte Reload
                                        # xmm3 = mem[0],zero,zero,zero
	mulss	%xmm3, %xmm3
	addss	%xmm1, %xmm3
	mulss	%xmm2, %xmm3
	mulss	%xmm0, %xmm3
	mulss	%xmm4, %xmm0
	mulss	%xmm9, %xmm0
	addss	%xmm10, %xmm0
	addss	%xmm11, %xmm0
	subss	%xmm0, %xmm15
	addss	%xmm5, %xmm6
	addss	%xmm6, %xmm3
	movss	140(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm0, %xmm0
	movss	144(%rsp), %xmm1                # 4-byte Reload
                                        # xmm1 = mem[0],zero,zero,zero
	mulss	%xmm1, %xmm1
	addss	%xmm0, %xmm1
	movss	148(%rsp), %xmm0                # 4-byte Reload
                                        # xmm0 = mem[0],zero,zero,zero
	mulss	%xmm0, %xmm0
	addss	%xmm1, %xmm0
	mulss	%xmm2, %xmm0
	mulss	%xmm4, %xmm0
	addss	%xmm3, %xmm0
	addss	%xmm15, %xmm0
	callq	__print_float@PLT
	movl	$10, %edi
	callq	__print_char@PLT
	movq	%rbx, 248(%rsp)
	xorl	%eax, %eax
	addq	$416, %rsp                      # imm = 0x1A0
	popq	%rbx
	retq
.Lfunc_end6:
	.size	main, .Lfunc_end6-main
                                        # -- End function
	.type	.LSTR_READFILE_ERR,@object      # @STR_READFILE_ERR
	.section	.rodata.str1.1,"aMS",@progbits,1
.LSTR_READFILE_ERR:
	.asciz	"file not found"
	.size	.LSTR_READFILE_ERR, 15

	.type	dpy,@object                     # @dpy
	.section	.rodata,"a",@progbits
	.globl	dpy
	.p2align	2, 0x0
dpy:
	.long	0x43b69eb8                      # float 365.23999
	.size	dpy, 4

	.type	dt,@object                      # @dt
	.globl	dt
	.p2align	2, 0x0
dt:
	.long	0x3c23d70a                      # float 0.00999999977
	.size	dt, 4

	.type	m0,@object                      # @m0
	.globl	m0
	.p2align	2, 0x0
m0:
	.long	0x421de9e6                      # float 39.4784164
	.size	m0, 4

	.type	m1,@object                      # @m1
	.globl	m1
	.p2align	2, 0x0
m1:
	.long	0x3d1a64af                      # float 0.0376936756
	.size	m1, 4

	.type	m2,@object                      # @m2
	.globl	m2
	.p2align	2, 0x0
m2:
	.long	0x3c38ea48                      # float 0.0112863258
	.size	m2, 4

	.type	m3,@object                      # @m3
	.globl	m3
	.p2align	2, 0x0
m3:
	.long	0x3ae1ee95                      # float 0.00172372407
	.size	m3, 4

	.type	m4,@object                      # @m4
	.globl	m4
	.p2align	2, 0x0
m4:
	.long	0x3b05479b                      # float 0.00203368696
	.size	m4, 4

	.type	pi,@object                      # @pi
	.globl	pi
	.p2align	2, 0x0
pi:
	.long	0x40490fdb                      # float 3.14159274
	.size	pi, 4

	.type	.Lstr.0,@object                 # @str.0
	.p2align	3, 0x0
.Lstr.0:
	.zero	9
	.size	.Lstr.0, 9

	.type	.Lstr.1,@object                 # @str.1
	.p2align	3, 0x0
.Lstr.1:
	.quad	5                               # 0x5
	.asciz	"BOUND"
	.size	.Lstr.1, 14

	.type	.Lll_empty_list,@object         # @ll_empty_list
	.section	.rodata.cst16,"aM",@progbits,16
	.p2align	3, 0x0
.Lll_empty_list:
	.zero	16
	.size	.Lll_empty_list, 16

	.globl	solar_mass
.set solar_mass, m0
	.section	".note.GNU-stack","",@progbits
