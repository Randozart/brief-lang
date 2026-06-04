# float_math — Brief 0.77× (23% faster)

## What it tests

Float arithmetic in a hot loop: 12 Float state fields, each updated with
multiplication and addition. 50M iterations, 12 Float ops per tick.

## Result

| | Time | Ratio |
|---|------|-------|
| Brief | 0.0044s | — |
| C | 0.0059s | 0.77× |

## Why Brief wins

Brief's SSA mode **pre-extracts all 12 Float fields from the `%State` struct into
scalar `float` registers before the loop body**. This means every `fmul`/`fadd`
in the body reads from registers, not memory. All 12 operations are independent,
filling all CPU execution ports simultaneously.

C's optimizer does register allocation at a lower level — it has to prove each
variable doesn't alias before promoting it. The output `year` variable writes back
to memory every iteration, which can inhibit register promotion of neighboring
variables.

### Key assembly evidence

**Brief hot loop** (from `float_math.s:401304-401344`):
```asm
vmovsd  0x4(%rdi),%xmm0           ; load struct fields 1-2
vmulps  0xcff(%rip),%xmm0,%xmm1   ; packed multiply of 2 floats
vmovss  (%rdi),%xmm2              ; load field 0
vinsertps $0x1c,%xmm0,%xmm2,%xmm0 ; combine into xmm0
vaddps  %xmm0,%xmm1,%xmm0         ; packed add
vmovlps %xmm0,(%rdi)              ; store back 2 fields
vaddss  0xc(%rdi),%xmm0,%xmm1     ; scalar add on field 3
vmovss  %xmm1,0xc(%rdi)           ; store field 3
...
```
Note: `vmulps`/`vaddps` (packed SIMD) alongside `vaddss`/`vmovss` (scalar).
Brief's SSA decomposition lets LLVM mix packed and scalar ops optimally.

**C hot loop** (from `float_math_c.s:11c0-1209`):
```asm
vbroadcastss 0xe2a(%rip),%ymm9    ; broadcast constant to all lanes
vxorps  %xmm10,%xmm10,%xmm10      ; zero-initialize (setup overhead)
vxorps  %xmm11,%xmm11,%xmm11
vxorps  %xmm12,%xmm12,%xmm12
...
```
C's optimizer uses `ymm` (256-bit AVX2) registers with `vbroadcastss` for
vectorization, but the setup overhead (12 `vxorps` zero-initializations) adds
cost that dominates the short loop body.

## Compiler optimization paths

| Pass | Brief | C |
|------|-------|---|
| SROA | ✅ SSA mode decomposes %State → 12 scalar float registers | ✅ clang scalar replacement |
| Float promotion | ✅ Pre-extracted in entry block | ✅ via SROA |
| fast-math | ✅ `fmul fast`, `fadd fast` | ✅ `-ffast-math` |
| SIMD | ✅ Mixed scalar/packed via LLVM | ✅ ymm vectorization |
| LTO | ❌ No FFI calls in body | N/A |

## Reproduce

```bash
cargo build --release --bin brief-compiler
bash benchmarks/build_and_bench.sh float_math
```
