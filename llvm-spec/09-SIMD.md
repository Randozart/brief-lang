# SIMD Vectorization

## Array Types → Vector Operations

Briev SIMD arrays (`Float[64]`, `Int[16]`) are memory-mapped with explicit alignment. The LLVM backend emits vectorized operations using LLVM's native vector types.

## Vector Load/Store

```briev
let data: Float[64] @ 0x40000000;
let result = data[0] + data[1];
```

```llvm
; Address computation
%base = inttoptr i64 0x40000000 to <4 x float>*

; Vector load (4 floats at once, 16-byte aligned)
%vec = load <4 x float>, <4 x float>* %base, align 16

; Extract individual elements (when needed)
%e0 = extractelement <4 x float> %vec, i32 0
%e1 = extractelement <4 x float> %vec, i32 1

; Vector operation
%sum = fadd float %e0, %e1
```

## Explicit Loop Vectorization

```briev
let i: Int = 0;
[i < 16] {
    &result[i] = input[i] * 2.0;
    &i = i + 1;
};
```

```llvm
; Scalar loop (for reference):
; loop:
;   %i = phi i64 [0, %entry], [%next, %loop]
;   %ptr = getelementptr float, float* %input, i64 %i
;   %val = load float, float* %ptr
;   %mul = fmul float %val, 2.0
;   %out_ptr = getelementptr float, float* %result, i64 %i
;   store float %mul, float* %out_ptr
;   %next = add i64 %i, 1
;   %cond = icmp slt i64 %next, 16
;   br i1 %cond, label %loop, label %exit, !llvm.loop !0

; With vectorization metadata:
!0 = !{!0, !1, !2}
!1 = !{!"llvm.loop.vectorize.enable", i1 true}
!2 = !{!"llvm.loop.interleave.count", i32 4}
```

With this metadata, LLVM's loop vectorizer produces:

```llvm
; Vectorized loop body:
%vec_in = load <4 x float>, <4 x float>* %ptr, align 16
%vec_mul = fmul <4 x float> %vec_in, <float 2.0, float 2.0, float 2.0, float 2.0>
store <4 x float> %vec_mul, <4 x float>* %out_ptr, align 16
```

## Alignment Rules

| Element Count | Vector Width | Alignment | ISA |
|--------------|--------------|-----------|-----|
| 2-4 | `<4 x float>` / `<4 x i32>` | 16 | SSE |
| 5-8 | `<8 x float>` / `<8 x i32>` | 32 | AVX2 |
| 9-16 | `<16 x float>` / `<16 x i32>` | 64 | AVX-512 |

## Implementation

The backend does NOT auto-vectorize. Instead, it:

1. Emits scalar loops for array operations
2. Attaches `!llvm.loop.vectorize.enable` metadata to loop branches
3. LLVM's vectorizer handles the rest

This keeps the backend simple while getting full SIMD performance from LLVM.