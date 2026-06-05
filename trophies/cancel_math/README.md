# cancel_math — Brief 0.73× (27% faster)

## What it tests

Algebraic simplification: `x = x + (R + 1) - R` is rewritten by `simplify_body` to
`x = x + 1`, then `detect_increments` finds the counter increment. 50M iterations
with cumulative sum (acc) and conditional `__print_int` every 5M ticks.

## Result

| | Time | Ratio |
|---|------|-------|
| Brief | 0.0410s | — |
| C | 0.0555s | 0.73× |

## Why Brief wins

**Algebraic simplification removes expression overhead.** The Brief body
`&count = count + (R + 1 - R)` passes through `simplify_body` which applies
the rule `(a + b) - a → b`, reducing the RHS to `count + 1`. The compiler
sees a pure counter increment and emits the same `add` as C's `count++`.

**LTO inlines `__print_int` into the hot loop.** Same mechanism as the
print_loop trophy: `clang -c -emit-llvm` compiles `brief_rt.c` to bitcode,
`llvm-link` merges it with the program IR, and `opt -O3` inlines the
`fprintf(stderr, ...)` call. C's `fprintf` goes through the PLT.

**SSA mode avoids state struct load/store.** The compiler detects the
convergent contract `[count < N][count == N]`, proves the body is pure
(only mutates the bounded counter and a dead-field `acc` that becomes a
register), and promotes `count` and `acc` to scalar registers. No
pointer-based state struct access.

### Key assembly evidence

**Brief** — Count and acc are in registers, `__print_int` is inlined:
```asm
; count and acc live in registers throughout the loop
add    %rbx,%r12              ; acc += count (in regs)
add    $0x1,%rbx              ; count++ (in reg)
; print guard uses multiply-by-reciprocal
movabs $0x6f05b59d3b200000,%rdx
imul   %rbx,%rdx
```

**C** — Same patterns, but `fprintf` is an out-of-line PLT call:
```asm
call   fprintf@PLT            ; PLT dispatch each print
```

## Compiler optimization paths

| Pass | Brief | C |
|------|-------|---|
| Algebraic simplify | ✅ `(R+1)-R → 1` via `simplify_body` | N/A — no expression to simplify |
| Counter detection | ✅ `detect_increments` finds delta=1 | ✅ `count++` native |
| LTO inlines FFI | ✅ `__print_int` inlined | ❌ `fprintf` varargs via PLT |
| SSA mode | ✅ Scalar registers, no state struct | ✅ Standard while-loop |

## Reproduce

```bash
cargo build --release --bin brief-compiler
bash benchmarks/build_and_bench.sh cancel_math
```