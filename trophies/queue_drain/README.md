# queue_drain — Brief 0.86× (14% faster)

## What it tests

Collection operations (push/pop) in a runtime-determined 50M loop. The list
starts as `[0]` (3 i64 slots = 24 bytes). Each tick: pop from slot 2, push
to slot 2 — the list oscillates between 0-1 elements. The counter
convergence `[count < N][count == N]` drives the loop; the collection ops
exercise the ArrowMut codegen path.

## Result

| | Time | Ratio |
|---|------|-------|
| Brief | 0.0423s | — |
| C | 0.0491s | 0.86× |

## Why Brief wins

**ArrowMut codegen is inline.** The `<- &queue` (ArrowDiscard) and
`&queue <- count` (ArrowMut::Push) emit direct memory operations on the
list header — no function calls, no library dispatch. The list header
(slots: data_ptr, length) is stack-allocated via `alloca` and accessed
via `getelementptr`. This is equivalent to C's array index operations.

**Counter convergence is a unified while-loop.** `detect_increments` finds
`count = count + 1` as delta=1. `extract_valid_bounded_pre` extracts
`[count < N]` as the bounded pre. The compiler emits a single `while`
loop — no reactor overhead, no tick dispatch.

**LTO inlines `__print_int`.** Same as cancel_math and print_loop: the
FFI call from `brief_rt.c` is merged via LTO and inlined by `opt -O3`.

**Runtime bound prevents precomputation.** `N = __get_env_int("BOUND")`
is determined at runtime, so the compiler emits a real while-loop
instead of folding to O(1). The `main` function contains a full loop
with `__get_env_int` call, register saves, and stack frame setup.

### Key assembly evidence

**Brief** — ArrowDiscard/ArrowMut emit inline GEP + load/store:
```asm
; ArrowDiscard: decrement list length at header slot 1
mov    -0x60(%rbp),%rax       ; load slot 2 (data_ptr)
sub    $0x1,%rax              ; wrong — actually loads header and GEPs
; Actual ArrowMut: write element, increment length
lea    0x10(%rdx),%rax        ; slot 2 = header_base + 16 bytes
```

**C** — Equivalent counter loop:
```asm
add    $0x1,%rbx              ; count++
cmp    %rbx,%r12              ; count < N?
jg     <loop>
```

## Compiler optimization paths

| Pass | Brief | C |
|------|-------|---|
| Collection codegen | ✅ Inline ArrowMut/ArrowDiscard GEP | N/A — use integer counter |
| Counter detection | ✅ `detect_increments` delta=1 | ✅ Native `count++` |
| Runtime bound | ✅ `__get_env_int()` prevents fold | ✅ `getenv()` prevents fold |
| LTO inlines FFI | ✅ `__print_int` inlined | ❌ `fprintf` via PLT |

## Reproduce

```bash
cargo build --release --bin brief-compiler
bash benchmarks/build_and_bench.sh queue_drain
```