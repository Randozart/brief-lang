# GPU I/O Intrinsics — Observable Output Without `frgn`

**Date:** 2026-06-18  
**Problem:** `.abv` bans `frgn`, but kernels need observable output for liveness.
The solution is not `frgn` — it's existing I/O intrinsics (`print_int#`,
`print_float#`, `put_char#`) wired into the SPIR-V backend.

## Precept

**Zero `frgn` in `.abv`.** Every operation maps to an `Intrinsic` variant.
If no suitable intrinsic exists, add one — never reach for `frgn`.

## Approach

### 1. Wire I/O intrinsics into SPIR-V eligibility

Add `PrintInt`, `PutChar`, `PrintFloat` to `is_gpu_safe_intrinsic` in
`gpu.rs`. Also add `PrintFloat` to `is_float_context`.

### 2. SPIR-V codegen for I/O intrinsics

Each I/O intrinsic writes to a dedicated **print buffer** (an additional
`i8*` SPIR-V kernel parameter, slot 2, after `%in_buf` and `%out_buf`):

| Intrinsic | SPIR-V LLVM IR |
|-----------|----------------|
| `print_int#(n)` | `store i64 %n, i8* %print_base, align 8` |
| `print_float#(f)` | `store float %f, float* bitcast i8* %print_base to float*, align 4` |
| `put_char#(c)` | `store i8 %c, i8* %print_base, align 1` |

`%print_base = GEP i8, i8* %print_buf, i64 %gtid`

The kernel signature becomes:
```llvm
define spir_kernel void @kernel(i8* %in_buf, i8* %out_buf, i8* %print_buf, i64 %N)
```

The print buffer is ONLY emitted when the kernel body uses print intrinsics
(scan body before emission).

### 3. Host runtime drain

In `briv_gpu_launch`, after kernel dispatch + buffer readback:

```c
// Drain print buffer
if (has_print_buffer) {
    int64_t* print_data = (int64_t*)host_ptr_of_print_buf;
    for (int64_t i = 0; i < N; i++) {
        if (print_data[i] != 0)
            printf("%ld\n", print_data[i]);
    }
}
```

The C runtime allocates `N * 8` bytes for the print buffer when print
intrinsics are detected.

### 4. `.abv` test file

```briv
// Pure GPU kernel — zero frgn
node increment
    [i < N]
    [i == N]
{
    let idx: Int = get_global_id#(0);
    [idx < N] { &i = i + 1; };
    term;
};
term! -> print_int#(i);    // intrinsic, NOT frgn
```

### 5. Typechecker stays strict

`ForeignBinding` in `.gbv` → `Error` (G001). No exceptions.

## Files to change

| File | Change |
|------|--------|
| `src/backend/llvm/gpu.rs` | Add PrintInt/PutChar/PrintFloat to allowlist + `emit_spirv_intrinsic` + print buffer generation |
| `lib/runtime/briv_gpu_rt.c` | Allocate + drain print buffer in `briv_gpu_launch` |
| `AGENTS.md` | Add "before frgn, check intrinsics" directive |
| `test_abv.abv` | Clean test with print_int# intrinsic |

## Incremental test

```bash
echo 'N=100 ./test_abv'   # Should print 100
```

The host reads the print buffer after GPU dispatch and calls `printf`
for each non-zero value.
